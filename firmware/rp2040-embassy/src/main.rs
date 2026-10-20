#![no_std]
#![no_main]

//! # uPLC Studio - RP2040 Embassy Micro-VM Firmware
//!
//! Architecture:
//! - **Task 1 (`plc_scan_task`)**: Deterministic 10ms PLC Scan Cycle (`Ticker`)
//!   1. Sample physical GPIO inputs `X0..X3` (`GP10..GP13`) -> `vm.bit_mem[0x00..0x03]`
//!   2. Run `vm.step_scan(SCAN_CYCLE_MS)`
//!   3. Drive physical GPIO outputs `Y0..Y5` (`GP14..GP15, GP6..GP7, GP2..GP3`) <- `vm.bit_mem[0x20..0x25]`
//! - **Task 2 (`usb_cdc_comm_task`)**: Asynchronous USB-Serial Frame Receiver
//!   - Listens for packets sent from uPLC Studio's **"⚡ Update to PLC"** menu:
//!     `[SOF: 0xAA] [CMD: 1B] [LEN: 2B LE] [PAYLOAD: 4*N B] [CRC8: 1B]`
//!   - Hot-reloads the bytecode into `SHARED_VM` and persists it to RP2040 Flash!

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::flash::{Async, Flash, ERASE_SIZE};
use embassy_rp::gpio::{Input, Output};
use embassy_rp::peripherals::{FLASH, UART0, UART1, USB};
use embassy_rp::uart::{self, BufferedInterruptHandler, BufferedUart};
use embassy_rp::usb::{Driver as UsbDriver, InterruptHandler as UsbInterruptHandler};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Instant, Ticker};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State as CdcAcmState};
use embassy_usb::{Builder as UsbBuilder, Config as UsbConfig, UsbDevice};
use static_cell::StaticCell;
use uplc_vm::{
    MicroVm, CMD_DOWNLOAD_PROGRAM, CMD_FORCE_IO, CMD_FORCE_IO_RESPONSE, CMD_MONITOR_STATUS,
    CMD_RUN_STOP, CMD_STEP, CMD_UPLOAD_PROGRAM, CMD_WRITE_VALUE, CMD_WRITE_VALUE_RESPONSE,
    CMD_PERSIST_SAVE, CMD_PERSIST_SAVE_RESPONSE, CMD_PERSIST_CLEAR, CMD_PERSIST_CLEAR_RESPONSE,
    MAX_INSTRUCTIONS, PACKET_SOF,
};
use panic_halt as _;

mod config;
use config::*;
use portable_atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

/// Global PLC Error Flag for LED and system diagnostics
static PLC_HAS_ERROR: AtomicBool = AtomicBool::new(false);

/// CRC16 of the RETAIN region (`M64..M95`, `D96..D127`) as last saved in Flash
static RETAIN_SAVED_CRC: AtomicU16 = AtomicU16::new(0);

/// Total number of RETAIN Flash saves (`%SD7` / `D135`)
static RETAIN_SAVE_COUNT: AtomicU32 = AtomicU32::new(0);

/// Whether the RETAIN region in RAM differs from Flash (`%SM7` / `M103`)
static RETAIN_DIRTY: AtomicBool = AtomicBool::new(false);

/// Async channel for triggering background Flash saves of RETAIN memory without holding the VM mutex
static FLASH_CMD_CHANNEL: Channel<CriticalSectionRawMutex, ([u8; 32], [i32; 32]), 2> =
    Channel::new();

/// Global Shared Micro-VM protected by an Embassy async-friendly Mutex
static SHARED_VM: Mutex<CriticalSectionRawMutex, MicroVm> = Mutex::new(MicroVm::new());

/// Global Shared Flash handle so USB-CDC Serial can persist uploaded Bytecode
static SHARED_FLASH: Mutex<
    CriticalSectionRawMutex,
    Option<Flash<'static, FLASH, Async, FLASH_SIZE_BYTES>>,
> = Mutex::new(None);

bind_interrupts!(struct Irqs {
    UART0_IRQ => BufferedInterruptHandler<UART0>;
    UART1_IRQ => BufferedInterruptHandler<UART1>;
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    for pin_idx in 0..30 {
        embassy_rp::pac::PADS_BANK0.gpio(pin_idx).modify(|w| {
            w.set_ie(true);
            w.set_pde(true);
            w.set_pue(false);
        });
    }
    cortex_m::asm::delay(10_000);
    let boot_gpio_mask = embassy_rp::pac::SIO.gpio_in(0).read();
    info!("====================================================");
    info!("uPLC Studio - RP2040 Micro-VM Firmware Initialized!");
    info!(
        "Scan Cycle: {} ms | Boot High GPIOs: 0x{:08X}",
        SCAN_CYCLE_MS, boot_gpio_mask
    );
    info!("PLC Status LED: GP25 [RUN=Heartbeat/Solid | STOP=Slow Beacon | ERROR=Fast Strobe]");
    info!("====================================================");

    // -------------------------------------------------------------------------
    // 1. Configure Hardware Digital Inputs & Outputs (from config.rs)
    // -------------------------------------------------------------------------
    let hw_io = configure_hardware_io!(p);

    // -------------------------------------------------------------------------
    // 3. Initialize Flash Storage & Load Saved Bytecode & Persistent Data on Boot
    // -------------------------------------------------------------------------
    let mut flash = Flash::<_, Async, FLASH_SIZE_BYTES>::new(p.FLASH, p.DMA_CH0);
    load_program_from_flash(&mut flash).await;
    {
        let mut bit_latch = [0u8; 32];
        let mut word_latch = [0i32; 32];
        if load_persistent_data_from_flash(&mut flash, &mut bit_latch, &mut word_latch) {
            let mut vm = SHARED_VM.lock().await;
            vm.bit_mem[LATCH_BIT_START_ADDR as usize..LATCH_BIT_START_ADDR as usize + 32]
                .copy_from_slice(&bit_latch);
            vm.prev_bit_mem[LATCH_BIT_START_ADDR as usize..LATCH_BIT_START_ADDR as usize + 32]
                .copy_from_slice(&bit_latch);
            vm.word_mem[LATCH_WORD_START_ADDR as usize..LATCH_WORD_START_ADDR as usize + 32]
                .copy_from_slice(&word_latch);
        }
        RETAIN_SAVED_CRC.store(
            compute_retain_crc16(&bit_latch, &word_latch),
            Ordering::Relaxed,
        );
        RETAIN_DIRTY.store(false, Ordering::Relaxed);
    }
    {
        let mut guard = SHARED_FLASH.lock().await;
        *guard = Some(flash);
    }
    spawner.spawn(flash_retain_task()).unwrap();

    // -------------------------------------------------------------------------
    // 4. Configure Native USB CDC-ACM Virtual Serial Port (/dev/ttyACM*)
    // -------------------------------------------------------------------------
    let usb_driver = UsbDriver::new(p.USB, Irqs);
    let mut usb_config = UsbConfig::new(0x2E8A, 0x000A);
    usb_config.manufacturer = Some("PksofttecH");
    usb_config.product = Some("uPLC Studio RP2040 Micro-VM");
    usb_config.serial_number = Some("UPLC-RP2040-001");
    usb_config.max_power = 100;
    usb_config.max_packet_size_0 = 64;

    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static CDC_STATE: StaticCell<CdcAcmState> = StaticCell::new();

    let mut builder = UsbBuilder::new(
        usb_driver,
        usb_config,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        &mut [],
        CONTROL_BUF.init([0; 64]),
    );

    let cdc_class = CdcAcmClass::new(&mut builder, CDC_STATE.init(CdcAcmState::new()), 64);
    let usb_dev = builder.build();

    spawner.spawn(usb_device_task(usb_dev)).unwrap();
    spawner.spawn(usb_cdc_comm_task(cdc_class)).unwrap();

    // -------------------------------------------------------------------------
    // 5. Configure Buffered UART1 for Modbus RTU RS-485
    //    GP8 = TX, GP9 = RX, DE/RE = None (Auto-direction hardware)
    // -------------------------------------------------------------------------
    static MB_TX_BUF: StaticCell<[u8; 256]> = StaticCell::new();
    static MB_RX_BUF: StaticCell<[u8; 512]> = StaticCell::new();
    let mut mb_uart_cfg = uart::Config::default();
    mb_uart_cfg.baudrate = MODBUS_BAUD_RATE;

    let mb_uart = BufferedUart::new(
        p.UART1,
        Irqs,
        p.PIN_8,
        p.PIN_9,
        MB_TX_BUF.init([0; 256]),
        MB_RX_BUF.init([0; 512]),
        mb_uart_cfg,
    );

    // Spawn Modbus RTU Master Task (DE/RE = None)
    spawner.spawn(modbus_master_task(mb_uart, None)).unwrap();

    // -------------------------------------------------------------------------
    // 5b. Configure Buffered UART0 for User UART_SEND / UART_RECV Function Blocks
    //     GP0 = TX, GP1 = RX @ 115200 8N1
    // -------------------------------------------------------------------------
    static USER_UART_TX_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static USER_UART_RX_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    let mut user_uart_cfg = uart::Config::default();
    user_uart_cfg.baudrate = USER_UART_BAUD;

    let user_uart = BufferedUart::new(
        p.UART0,
        Irqs,
        p.PIN_0,
        p.PIN_1,
        USER_UART_TX_BUF.init([0; 64]),
        USER_UART_RX_BUF.init([0; 64]),
        user_uart_cfg,
    );
    spawner.spawn(uart_task(user_uart)).unwrap();

    // -------------------------------------------------------------------------
    // 6. Configure Hardware ADC for Analog Inputs AI0..AI3 (%IW0..%IW3)
    //    GP26=ADC0, GP27=ADC1, GP28=ADC2, ADC_TEMP_SENSOR=On-chip Die Temp
    // -------------------------------------------------------------------------
    let adc_hw = configure_hardware_adc!(p);

    // Spawn the Deterministic PLC Scan Cycle Task
    spawner
        .spawn(plc_scan_task(hw_io.inputs, hw_io.outputs, hw_io.status_led, Some(adc_hw)))
        .unwrap();
}

/// Background USB Device Runner Task
#[embassy_executor::task]
async fn usb_device_task(mut usb: UsbDevice<'static, UsbDriver<'static, USB>>) {
    usb.run().await;
}

/// Native USB CDC-ACM Serial Packet Receiver Task (`SOF 0xAA | CMD | LEN | PAYLOAD | CRC8`)
#[embassy_executor::task]
async fn usb_cdc_comm_task(mut class: CdcAcmClass<'static, UsbDriver<'static, USB>>) {
    static USB_FRAME_BUF: StaticCell<[u8; MAX_INSTRUCTIONS * 4 + 8]> = StaticCell::new();
    let frame_buf = USB_FRAME_BUF.init([0u8; MAX_INSTRUCTIONS * 4 + 8]);
    static USB_STATUS_BUF: StaticCell<[u8; uplc_vm::STATUS_PACKET_LEN]> = StaticCell::new();
    let status_buf = USB_STATUS_BUF.init([0u8; uplc_vm::STATUS_PACKET_LEN]);

    loop {
        class.wait_connection().await;
        info!("USB-CDC Serial Port Connected!");

        let mut rx_pos: usize = 0;
        let mut expected_len: Option<usize> = None;
        let mut chunk = [0u8; 64];

        while let Ok(n) = class.read_packet(&mut chunk).await {
            for &b in &chunk[..n] {
                if rx_pos == 0 {
                    if b == PACKET_SOF {
                        frame_buf[0] = PACKET_SOF;
                        rx_pos = 1;
                        expected_len = None;
                    }
                    continue;
                }

                if rx_pos < frame_buf.len() {
                    frame_buf[rx_pos] = b;
                    rx_pos += 1;
                } else {
                    rx_pos = 0;
                    expected_len = None;
                    continue;
                }

                if rx_pos == 4 {
                    let payload_len = u16::from_le_bytes([frame_buf[2], frame_buf[3]]) as usize;
                    if payload_len <= MAX_INSTRUCTIONS * 4 {
                        expected_len = Some(5 + payload_len);
                    } else {
                        rx_pos = 0;
                        expected_len = None;
                    }
                }

                if let Some(total_len) = expected_len {
                    if rx_pos == total_len {
                        let result = {
                            let mut vm = SHARED_VM.lock().await;
                            vm.process_serial_packet(&frame_buf[..total_len])
                        };

                        match result {
                            Ok(CMD_DOWNLOAD_PROGRAM) => {
                                let payload_len = total_len - 5;
                                info!(
                                    "USB-CDC CMD 0x01: Downloaded {} bytes ({} instructions) -> Saving to Flash...",
                                    payload_len,
                                    payload_len / 4
                                );
                                let save_ok = {
                                    let mut fl_guard = SHARED_FLASH.lock().await;
                                    if let Some(flash) = fl_guard.as_mut() {
                                        save_program_to_flash(flash, &frame_buf[4..4 + payload_len])
                                    } else {
                                        false
                                    }
                                };
                                if save_ok {
                                    PLC_HAS_ERROR.store(false, Ordering::Relaxed);
                                    let _ = class.write_packet(&[PACKET_SOF, 0x81, 0x00]).await;
                                } else {
                                    PLC_HAS_ERROR.store(true, Ordering::Relaxed);
                                    let _ = class.write_packet(&[PACKET_SOF, 0xFF, 0x02]).await;
                                }
                            }
                            Ok(CMD_RUN_STOP) => {
                                PLC_HAS_ERROR.store(false, Ordering::Relaxed);
                                info!("USB-CDC CMD 0x02: Run/Stop toggled!");
                                let should_save = {
                                    let vm = SHARED_VM.lock().await;
                                    !vm.running
                                };
                                if should_save {
                                    let (bit_latch, word_latch) = {
                                        let vm = SHARED_VM.lock().await;
                                        let mut b = [0u8; 32];
                                        let mut w = [0i32; 32];
                                        b.copy_from_slice(&vm.bit_mem[LATCH_BIT_START_ADDR as usize..LATCH_BIT_START_ADDR as usize + 32]);
                                        w.copy_from_slice(&vm.word_mem[LATCH_WORD_START_ADDR as usize..LATCH_WORD_START_ADDR as usize + 32]);
                                        (b, w)
                                    };
                                    let mut fl_guard = SHARED_FLASH.lock().await;
                                    if let Some(flash) = fl_guard.as_mut() {
                                        if save_persistent_data_to_flash(flash, &bit_latch, &word_latch) {
                                            let mut vm = SHARED_VM.lock().await;
                                            if ENABLE_SPECIAL_MARKERS {
                                                vm.bit_mem[SM_RETAIN_DIRTY as usize] = 0;
                                            }
                                            if ENABLE_SPECIAL_REGISTERS {
                                                vm.word_mem[SD_RETAIN_SAVE_COUNT as usize] =
                                                    RETAIN_SAVE_COUNT.load(Ordering::Relaxed) as i32;
                                            }
                                        }
                                    }
                                }
                                let _ = class.write_packet(&[PACKET_SOF, 0x82, 0x00]).await;
                            }
                            Ok(CMD_MONITOR_STATUS) => {
                                let len = {
                                    let vm = SHARED_VM.lock().await;
                                    vm.write_status_packet(status_buf).unwrap_or(0)
                                };
                                if len > 0 {
                                    let mut offset = 0;
                                    while offset < len {
                                        let chunk = (len - offset).min(64);
                                        let _ = class.write_packet(&status_buf[offset..offset + chunk]).await;
                                        offset += chunk;
                                    }
                                }
                            }
                            Ok(CMD_UPLOAD_PROGRAM) => {
                                let len = {
                                    let vm = SHARED_VM.lock().await;
                                    vm.write_program_packet(frame_buf).unwrap_or(0)
                                };
                                if len > 0 {
                                    let mut offset = 0;
                                    while offset < len {
                                        let chunk = (len - offset).min(64);
                                        let _ = class.write_packet(&frame_buf[offset..offset + chunk]).await;
                                        offset += chunk;
                                    }
                                }
                            }
                            Ok(CMD_STEP) => {
                                PLC_HAS_ERROR.store(false, Ordering::Relaxed);
                                info!("USB-CDC CMD 0x05: Executed Single Scan Step!");
                                let _ = class.write_packet(&[PACKET_SOF, 0x85, 0x00]).await;
                            }
                            Ok(CMD_FORCE_IO) => {
                                PLC_HAS_ERROR.store(false, Ordering::Relaxed);
                                info!("USB-CDC CMD 0x06: Force I/O state updated!");
                                let _ = class.write_packet(&[PACKET_SOF, CMD_FORCE_IO_RESPONSE, 0x00]).await;
                            }
                            Ok(CMD_WRITE_VALUE) => {
                                PLC_HAS_ERROR.store(false, Ordering::Relaxed);
                                info!("USB-CDC CMD 0x07: Write Value updated!");
                                let _ = class.write_packet(&[PACKET_SOF, CMD_WRITE_VALUE_RESPONSE, 0x00]).await;
                            }
                            Ok(CMD_PERSIST_SAVE) => {
                                PLC_HAS_ERROR.store(false, Ordering::Relaxed);
                                info!("USB-CDC CMD 0x08: Saving persistent data to Flash...");
                                let (bit_latch, word_latch) = {
                                    let vm = SHARED_VM.lock().await;
                                    let mut b = [0u8; 32];
                                    let mut w = [0i32; 32];
                                    b.copy_from_slice(&vm.bit_mem[LATCH_BIT_START_ADDR as usize..LATCH_BIT_START_ADDR as usize + 32]);
                                    w.copy_from_slice(&vm.word_mem[LATCH_WORD_START_ADDR as usize..LATCH_WORD_START_ADDR as usize + 32]);
                                    (b, w)
                                };
                                let ok = {
                                    let mut fl_guard = SHARED_FLASH.lock().await;
                                    if let Some(flash) = fl_guard.as_mut() {
                                        save_persistent_data_to_flash(flash, &bit_latch, &word_latch)
                                    } else {
                                        false
                                    }
                                };
                                if ok {
                                    {
                                        let mut vm = SHARED_VM.lock().await;
                                        if ENABLE_SPECIAL_MARKERS {
                                            vm.bit_mem[SM_RETAIN_DIRTY as usize] = 0;
                                        }
                                        if ENABLE_SPECIAL_REGISTERS {
                                            vm.word_mem[SD_RETAIN_SAVE_COUNT as usize] =
                                                RETAIN_SAVE_COUNT.load(Ordering::Relaxed) as i32;
                                        }
                                    }
                                    let _ = class.write_packet(&[PACKET_SOF, CMD_PERSIST_SAVE_RESPONSE, 0x00]).await;
                                } else {
                                    let _ = class.write_packet(&[PACKET_SOF, 0xFF, 0x08]).await;
                                }
                            }
                            Ok(CMD_PERSIST_CLEAR) => {
                                PLC_HAS_ERROR.store(false, Ordering::Relaxed);
                                info!("USB-CDC CMD 0x09: Clearing persistent data in Flash...");
                                let ok = {
                                    let mut fl_guard = SHARED_FLASH.lock().await;
                                    if let Some(flash) = fl_guard.as_mut() {
                                        clear_persistent_data_in_flash(flash)
                                    } else {
                                        false
                                    }
                                };
                                {
                                    let mut vm = SHARED_VM.lock().await;
                                    for i in 0..32 {
                                        vm.bit_mem[LATCH_BIT_START_ADDR as usize + i] = 0;
                                        vm.word_mem[LATCH_WORD_START_ADDR as usize + i] = 0;
                                    }
                                    if ENABLE_SPECIAL_MARKERS {
                                        vm.bit_mem[SM_RETAIN_DIRTY as usize] = 0;
                                    }
                                }
                                if ok {
                                    let _ = class.write_packet(&[PACKET_SOF, CMD_PERSIST_CLEAR_RESPONSE, 0x00]).await;
                                } else {
                                    let _ = class.write_packet(&[PACKET_SOF, 0xFF, 0x09]).await;
                                }
                            }
                            Err(e) => {
                                info!("USB-CDC Packet Error: {}", e);
                                PLC_HAS_ERROR.store(true, Ordering::Relaxed);
                                let _ = class.write_packet(&[PACKET_SOF, 0xFF, 0x01]).await;
                            }
                            _ => {}
                        }

                        rx_pos = 0;
                        expected_len = None;
                    }
                }
            }
        }
    }
}

/// Deterministic PLC Scan Cycle Task (`Read Inputs -> Execute Micro-VM -> Write Outputs -> Update Status LED`)
#[embassy_executor::task]
async fn plc_scan_task(
    inputs: [Option<Input<'static>>; 8],
    mut outputs: [Option<Output<'static>>; 8],
    mut status_led: Output<'static>,
    mut adc_hw: Option<HardwareAdc>,
) {
    let mut ticker = Ticker::every(Duration::from_millis(SCAN_CYCLE_MS as u64));
    let mut scan_div: u32 = 0;
    let mut led_ms: u32 = 0;
    let mut total_ms: u64 = 0;
    let mut min_scan_us: u32 = 0;
    let mut max_scan_us: u32 = 0;
    let mut last_retain_save_ms: u64 = 0;

    loop {
        ticker.next().await;
        total_ms = total_ms.wrapping_add(SCAN_CYCLE_MS as u64);
        led_ms = (led_ms + SCAN_CYCLE_MS) % 2000;

        let mut vm = SHARED_VM.lock().await;

        // 1. Sample Physical Digital Inputs X0..X7 into Unified Bit Memory (0x00..0x07)
        for (i, pin_opt) in inputs.iter().enumerate() {
            if let Some(pin) = pin_opt {
                let is_active = if INPUT_ACTIVE_LOW {
                    pin.is_low()
                } else {
                    pin.is_high()
                };
                vm.set_digital_input(i as u8, is_active);
            } else {
                vm.set_digital_input(i as u8, false);
            }
        }

        // 1.2 Sample Hardware Analog Inputs (%IW0..%IW3 / AI0..AI3)
        if ENABLE_ANALOG_INPUTS {
            if let Some(adc) = adc_hw.as_mut() {
                if let Ok(raw0) = adc.adc.blocking_read(&mut adc.ch_ai0) {
                    vm.word_mem[0] = raw0 as i32;
                }
                if let Ok(raw1) = adc.adc.blocking_read(&mut adc.ch_ai1) {
                    vm.word_mem[1] = raw1 as i32;
                }
                if let Ok(raw2) = adc.adc.blocking_read(&mut adc.ch_ai2) {
                    vm.word_mem[2] = raw2 as i32;
                }
                if let Ok(raw3) = adc.adc.blocking_read(&mut adc.ch_ai3) {
                    vm.word_mem[3] = raw3 as i32;
                    // RP2040 On-Chip Temp Sensor: Vbe = raw * 3.3 / 4096.0
                    // T = 27.0 - (Vbe - 0.706) / 0.001721
                    // Scaled to 0.1 °C integer units: 270 - ((vbe_uv - 706000) / 1721)
                    let vbe_uv = (raw3 as u32 * ADC_VREF_MV * 1000) / (ADC_MAX_RAW as u32 + 1);
                    let temp_c01 = 270i32 - (((vbe_uv as i32) - 706_000) / 1721);
                    if ENABLE_SPECIAL_REGISTERS {
                        vm.word_mem[SD_CPU_TEMP_C01 as usize] = temp_c01;
                    }
                }
            }
        }

        // 1.5 Update Special System Markers (%SM) prior to scan execution
        if ENABLE_SPECIAL_MARKERS {
            vm.bit_mem[SM_ALWAYS_ON as usize] = 1;
            vm.bit_mem[SM_ALWAYS_OFF as usize] = 0;
            vm.bit_mem[SM_FIRST_SCAN as usize] = if vm.scan_count == 0 { 1 } else { 0 };
            vm.bit_mem[SM_RETAIN_DIRTY as usize] = if RETAIN_DIRTY.load(Ordering::Relaxed) { 1 } else { 0 };
            vm.bit_mem[SM_CLOCK_10MS as usize] = if (total_ms % 20) < 10 { 1 } else { 0 };
            vm.bit_mem[SM_CLOCK_100MS as usize] = if (total_ms % 200) < 100 { 1 } else { 0 };
            vm.bit_mem[SM_CLOCK_1S as usize] = if (total_ms % 2000) < 1000 { 1 } else { 0 };
            vm.bit_mem[SM_CLOCK_1MIN as usize] = if (total_ms % 120_000) < 60_000 { 1 } else { 0 };
            vm.bit_mem[SM_SYSTEM_ERROR as usize] = if PLC_HAS_ERROR.load(Ordering::Relaxed) { 1 } else { 0 };
        }
        if ENABLE_SPECIAL_REGISTERS {
            vm.word_mem[SD_WATCHDOG_PRESET as usize] = DEFAULT_WATCHDOG_TIMEOUT_MS as i32;
            vm.word_mem[SD_SCAN_COUNT_LOW as usize] = (vm.scan_count & 0xFFFF) as i32;
            vm.word_mem[SD_SCAN_COUNT_HIGH as usize] = ((vm.scan_count >> 16) & 0xFFFF) as i32;
            vm.word_mem[SD_RETAIN_SAVE_COUNT as usize] = RETAIN_SAVE_COUNT.load(Ordering::Relaxed) as i32;
        }

        // 2. Execute 1 PLC Scan Cycle with Precise Microsecond Execution Measurement
        let scan_start = Instant::now();
        vm.step_scan(SCAN_CYCLE_MS);
        let last_scan_us = scan_start.elapsed().as_micros() as u32;

        if min_scan_us == 0 || last_scan_us < min_scan_us {
            min_scan_us = last_scan_us;
        }
        if last_scan_us > max_scan_us {
            max_scan_us = last_scan_us;
        }

        // 2.5 Update Diagnostic Special System Registers (%SD) and Overrun Flag
        let is_overrun = last_scan_us > (DEFAULT_WATCHDOG_TIMEOUT_MS * 1000);
        if ENABLE_SPECIAL_MARKERS {
            vm.bit_mem[SM_SCAN_OVERRUN as usize] = if is_overrun { 1 } else { 0 };
        }
        if ENABLE_SPECIAL_REGISTERS {
            vm.word_mem[SD_WATCHDOG_PRESET as usize] = DEFAULT_WATCHDOG_TIMEOUT_MS as i32;
            vm.word_mem[SD_CURRENT_SCAN_TIME_US as usize] = last_scan_us as i32;
            vm.word_mem[SD_MIN_SCAN_TIME_US as usize] = min_scan_us as i32;
            vm.word_mem[SD_MAX_SCAN_TIME_US as usize] = max_scan_us as i32;
            vm.word_mem[SD_SCAN_COUNT_LOW as usize] = (vm.scan_count & 0xFFFF) as i32;
            vm.word_mem[SD_SCAN_COUNT_HIGH as usize] = ((vm.scan_count >> 16) & 0xFFFF) as i32;
            vm.word_mem[SD_RETAIN_SAVE_COUNT as usize] = RETAIN_SAVE_COUNT.load(Ordering::Relaxed) as i32;
        }

        // 3. Update Physical Outputs Y0..Y7 from Unified Bit Memory (0x20..0x27)
        for (i, out_opt) in outputs.iter_mut().enumerate() {
            if let Some(out_pin) = out_opt {
                if vm.get_digital_output(i as u8) {
                    out_pin.set_high();
                } else {
                    out_pin.set_low();
                }
            }
        }

        // 4. Determine PLC Operating Status
        let has_error = PLC_HAS_ERROR.load(Ordering::Relaxed);
        let plc_status = if has_error {
            PlcStatus::Error
        } else if vm.running && vm.prog_len > 0 {
            PlcStatus::Run
        } else {
            PlcStatus::Stop
        };

        // 5. Drive Status LED (GP25) based on PLC status:
        // - RUN: Heartbeat 1 Hz (500ms ON / 500ms OFF) or Solid ON
        // - STOP: Slow Beacon (60ms pulse every 2000ms)
        // - ERROR: Fast Strobe (100ms ON / 100ms OFF = 5 Hz)
        let led_on = match plc_status {
            PlcStatus::Run => {
                if STATUS_LED_RUN_HEARTBEAT {
                    (led_ms % 1000) < 500
                } else {
                    true
                }
            }
            PlcStatus::Stop => led_ms < 60,
            PlcStatus::Error => (led_ms % 200) < 100,
        };
        if led_on {
            status_led.set_high();
        } else {
            status_led.set_low();
        }

        if vm.running && vm.prog_len > 0 {
            scan_div = scan_div.wrapping_add(1);
            // Check RETAIN area (M64..M95, D96..D127) every 1 second (100 scans at 10ms)
            if scan_div % 100 == 0 {
                let mut bit_latch = [0u8; 32];
                let mut word_latch = [0i32; 32];
                bit_latch.copy_from_slice(
                    &vm.bit_mem[LATCH_BIT_START_ADDR as usize..LATCH_BIT_START_ADDR as usize + 32],
                );
                word_latch.copy_from_slice(
                    &vm.word_mem[LATCH_WORD_START_ADDR as usize..LATCH_WORD_START_ADDR as usize + 32],
                );
                let current_crc = compute_retain_crc16(&bit_latch, &word_latch);
                let saved_crc = RETAIN_SAVED_CRC.load(Ordering::Relaxed);
                let dirty = current_crc != saved_crc;
                RETAIN_DIRTY.store(dirty, Ordering::Relaxed);
                if ENABLE_SPECIAL_MARKERS {
                    vm.bit_mem[SM_RETAIN_DIRTY as usize] = if dirty { 1 } else { 0 };
                }
                if dirty
                    && total_ms.saturating_sub(last_retain_save_ms)
                        >= RETAIN_AUTOSAVE_MIN_INTERVAL_S * 1000
                {
                    last_retain_save_ms = total_ms;
                    let _ = FLASH_CMD_CHANNEL.try_send((bit_latch, word_latch));
                }
            }
            if scan_div % 25 == 0 {
                info!(
                    "PLC Scan #{} | Status: {:?} | Prog: {} inst | ScanTime: {}us | Y0(GP15)={} Y1(GP14)={} | T0={}ms",
                    vm.scan_count,
                    plc_status,
                    vm.prog_len,
                    last_scan_us,
                    vm.bit_mem[0x20],
                    vm.bit_mem[0x21],
                    vm.word_mem[0xC0]
                );
            }
        }
    }
}


/// Restore saved PLC Bytecode from RP2040 Flash on power-up with CRC16 integrity check (or load Demo LED Blink program if empty/corrupted)
async fn load_program_from_flash(flash: &mut Flash<'static, FLASH, Async, FLASH_SIZE_BYTES>) {
    let mut header = [0u8; 12];
    if flash.blocking_read(FLASH_PLC_OFFSET, &mut header).is_ok() {
        let magic = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let byte_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let expected_crc = u16::from_le_bytes([header[8], header[9]]);

        if magic == FLASH_MAGIC
            && byte_len > 0
            && byte_len <= MAX_INSTRUCTIONS * 4
            && byte_len % 4 == 0
        {
            static LOAD_BUF: StaticCell<[u8; MAX_INSTRUCTIONS * 4]> = StaticCell::new();
            let buf = LOAD_BUF.init([0u8; MAX_INSTRUCTIONS * 4]);
            if flash
                .blocking_read(FLASH_PLC_OFFSET + 12, &mut buf[..byte_len])
                .is_ok()
            {
                let actual_crc = MicroVm::compute_crc16(&buf[..byte_len]);
                if actual_crc == expected_crc {
                    let mut vm = SHARED_VM.lock().await;
                    let _ = vm.load_bytecode(&buf[..byte_len]);
                    vm.running = true;
                    info!(
                        "Loaded saved PLC program from Flash ({} instructions, {} bytes, CRC16: 0x{:04X})",
                        byte_len / 4,
                        byte_len,
                        actual_crc
                    );
                    return;
                } else {
                    info!(
                        "Flash CRC16 mismatch: expected 0x{:04X}, computed 0x{:04X}. Corrupted flash, falling back to demo!",
                        expected_crc, actual_crc
                    );
                }
            }
        }
    }

    // Fallback default program on first boot:
    // example/08_rp2040_demo_led_blink.il (14 instructions, 56 bytes)
    // Blinks Demo LED Y0 (GP25) at 1 Hz (500ms ON / 500ms OFF) and Y1 (GP11) in opposite phase
    const DEMO_LED_PROG: [u8; 56] = [
        0x02, 0x01, 0x00, 0x00, // 0:  LDN  X1 (%IX0.1 STOP_PB)
        0x04, 0x41, 0x00, 0x00, // 1:  ANDN M1 (%MX0.1 M_CYCLE_RST)
        0x20, 0xC0, 0xF4, 0x01, // 2:  TON  T0, K500 (500ms ON Phase)
        0x07, 0x40, 0x00, 0x00, // 3:  ST   M0 (%MX0.0 M_HALF_PERIOD)
        0x01, 0x40, 0x00, 0x00, // 4:  LD   M0
        0x20, 0xC1, 0xF4, 0x01, // 5:  TON  T1, K500 (500ms OFF Phase)
        0x07, 0x41, 0x00, 0x00, // 6:  ST   M1
        0x02, 0x01, 0x00, 0x00, // 7:  LDN  X1
        0x04, 0x40, 0x00, 0x00, // 8:  ANDN M0
        0x05, 0x00, 0x00, 0x00, // 9:  OR   X0 (%IX0.0 START_PB Force ON)
        0x07, 0x20, 0x00, 0x00, // 10: ST   Y0 (%QX0.0 GP15)
        0x01, 0x40, 0x00, 0x00, // 11: LD   M0
        0x07, 0x21, 0x00, 0x00, // 12: ST   Y1 (%QX0.1 GP14)
        0xFF, 0x00, 0x00, 0x00, // 13: END
    ];
    let mut vm = SHARED_VM.lock().await;
    let _ = vm.load_bytecode(&DEMO_LED_PROG);
    vm.running = true;
    info!("Loaded built-in Demo LED Blink program (14 instructions, 56 bytes)");
}

/// Erase sector and save PLC Bytecode to RP2040 Flash with CRC16 header. Returns true if successful.
fn save_program_to_flash(
    flash: &mut Flash<'static, FLASH, Async, FLASH_SIZE_BYTES>,
    payload: &[u8],
) -> bool {
    if flash.blocking_erase(FLASH_PLC_OFFSET, FLASH_PLC_OFFSET + ERASE_SIZE as u32).is_err() {
        return false;
    }
    let mut page_buf = [0xFFu8; 256];
    let magic_bytes = FLASH_MAGIC.to_le_bytes();
    let len_bytes = (payload.len() as u32).to_le_bytes();
    let crc16 = MicroVm::compute_crc16(payload);
    let crc_bytes = crc16.to_le_bytes();

    page_buf[0..4].copy_from_slice(&magic_bytes);
    page_buf[4..8].copy_from_slice(&len_bytes);
    page_buf[8..10].copy_from_slice(&crc_bytes);
    page_buf[10..12].copy_from_slice(&[0u8, 0u8]); // reserved

    let first_chunk = payload.len().min(256 - 12);
    page_buf[12..12 + first_chunk].copy_from_slice(&payload[..first_chunk]);
    if flash.blocking_write(FLASH_PLC_OFFSET, &page_buf).is_err() {
        return false;
    }

    let mut offset = first_chunk;
    let mut flash_addr = FLASH_PLC_OFFSET + 256;
    while offset < payload.len() {
        let mut next_page = [0xFFu8; 256];
        let chunk = (payload.len() - offset).min(256);
        next_page[..chunk].copy_from_slice(&payload[offset..offset + chunk]);
        if flash.blocking_write(flash_addr, &next_page).is_err() {
            return false;
        }
        offset += chunk;
        flash_addr += 256;
    }
    info!("Saved program to Flash with CRC16: 0x{:04X}", crc16);
    true
}

/// Compute CRC16 over the RETAIN region (`M64..M95` = 32B + `D96..D127` = 128B LE)
fn compute_retain_crc16(bit_latch: &[u8; 32], word_latch: &[i32; 32]) -> u16 {
    let mut data_payload = [0u8; 160];
    data_payload[0..32].copy_from_slice(bit_latch);
    for (i, &w) in word_latch.iter().enumerate() {
        data_payload[32 + i * 4..32 + (i + 1) * 4].copy_from_slice(&w.to_le_bytes());
    }
    MicroVm::compute_crc16(&data_payload)
}

/// Load persistent Latch data (`M64..M95`, `D96..D127` / `%MW96..%MW127`) from RP2040 Flash Sector 511
fn load_persistent_data_from_flash(
    flash: &mut Flash<'static, FLASH, Async, FLASH_SIZE_BYTES>,
    bit_latch: &mut [u8; 32],
    word_latch: &mut [i32; 32],
) -> bool {
    let mut page_buf = [0u8; 256];
    if flash.blocking_read(FLASH_PERSISTENT_OFFSET, &mut page_buf).is_ok() {
        let magic = u32::from_le_bytes([page_buf[0], page_buf[1], page_buf[2], page_buf[3]]);
        if magic == FLASH_PERSISTENT_MAGIC {
            let version = u16::from_le_bytes([page_buf[4], page_buf[5]]);
            let expected_crc = u16::from_le_bytes([page_buf[6], page_buf[7]]);
            let data_payload = &page_buf[8..168];
            let actual_crc = MicroVm::compute_crc16(data_payload);
            if actual_crc == expected_crc && version == 1 {
                bit_latch.copy_from_slice(&data_payload[0..32]);
                for i in 0..32 {
                    let w = i32::from_le_bytes([
                        data_payload[32 + i * 4],
                        data_payload[32 + i * 4 + 1],
                        data_payload[32 + i * 4 + 2],
                        data_payload[32 + i * 4 + 3],
                    ]);
                    word_latch[i] = w;
                }
                info!("Loaded persistent Latch data from Flash (CRC16: 0x{:04X})", actual_crc);
                return true;
            }
        }
    }
    false
}

/// Save persistent Latch data (`M64..M95`, `D96..D127` / `%MW96..%MW127`) to RP2040 Flash Sector 511
fn save_persistent_data_to_flash(
    flash: &mut Flash<'static, FLASH, Async, FLASH_SIZE_BYTES>,
    bit_latch: &[u8; 32],
    word_latch: &[i32; 32],
) -> bool {
    if flash.blocking_erase(FLASH_PERSISTENT_OFFSET, FLASH_PERSISTENT_OFFSET + ERASE_SIZE as u32).is_err() {
        return false;
    }
    let mut page_buf = [0xFFu8; 256];
    let magic_bytes = FLASH_PERSISTENT_MAGIC.to_le_bytes();
    page_buf[0..4].copy_from_slice(&magic_bytes);
    page_buf[4..6].copy_from_slice(&1u16.to_le_bytes()); // version 1

    let mut data_payload = [0u8; 160];
    data_payload[0..32].copy_from_slice(bit_latch);
    for (i, &w) in word_latch.iter().enumerate() {
        data_payload[32 + i * 4..32 + (i + 1) * 4].copy_from_slice(&w.to_le_bytes());
    }

    let crc16 = MicroVm::compute_crc16(&data_payload);
    page_buf[6..8].copy_from_slice(&crc16.to_le_bytes());
    page_buf[8..168].copy_from_slice(&data_payload);

    if flash.blocking_write(FLASH_PERSISTENT_OFFSET, &page_buf).is_err() {
        return false;
    }
    RETAIN_SAVED_CRC.store(crc16, Ordering::Relaxed);
    RETAIN_DIRTY.store(false, Ordering::Relaxed);
    RETAIN_SAVE_COUNT.fetch_add(1, Ordering::Relaxed);
    info!("Saved persistent Latch data to Flash with CRC16: 0x{:04X}", crc16);
    true
}

/// Erase persistent Latch data in RP2040 Flash Sector 511 (Reset Origin)
fn clear_persistent_data_in_flash(
    flash: &mut Flash<'static, FLASH, Async, FLASH_SIZE_BYTES>,
) -> bool {
    if flash.blocking_erase(FLASH_PERSISTENT_OFFSET, FLASH_PERSISTENT_OFFSET + ERASE_SIZE as u32).is_err() {
        return false;
    }
    RETAIN_SAVED_CRC.store(
        compute_retain_crc16(&[0u8; 32], &[0i32; 32]),
        Ordering::Relaxed,
    );
    RETAIN_DIRTY.store(false, Ordering::Relaxed);
    info!("Cleared persistent Latch data in Flash (Sector 511 erased)");
    true
}

/// Background Flash Persistence Task for RETAIN auto-save (`M64..M95`, `D96..D127`)
#[embassy_executor::task]
async fn flash_retain_task() {
    loop {
        let (bit_latch, word_latch) = FLASH_CMD_CHANNEL.receive().await;
        let mut fl_guard = SHARED_FLASH.lock().await;
        if let Some(flash) = fl_guard.as_mut() {
            let _ = save_persistent_data_to_flash(flash, &bit_latch, &word_latch);
        }
    }
}

/// User UART0 Task (`GP0` TX / `GP1` RX @ 115200 8N1) for `UART_SEND` and `UART_RECV`
#[embassy_executor::task]
async fn uart_task(mut uart: BufferedUart<'static, UART0>) {
    use embedded_io_async::{Read, Write};

    info!(
        "User UART0 Task Ready (GP0=TX, GP1=RX @ {} baud 8N1)...",
        USER_UART_BAUD
    );

    let mut rx_ring = [0u8; 64];
    let mut ring_head: usize = 0;
    let mut ring_tail: usize = 0;
    let mut ring_len: usize = 0;
    let mut rx_chunk = [0u8; 16];

    loop {
        // 1. Transmit any pending UartSend job from MicroVm (without holding VM mutex across UART I/O)
        let pending_tx = {
            let mut vm = SHARED_VM.lock().await;
            vm.uart_tx_out.take()
        };
        if let Some(job) = pending_tx {
            let mut tx_buf = [0u8; 16];
            let n = job.encode(&mut tx_buf);
            let _ = uart.write_all(&tx_buf[..n]).await;
            let _ = uart.flush().await;
            let mut vm = SHARED_VM.lock().await;
            vm.uart_tx_busy = false;
        }

        // 2. Non-blocking / 2ms-window poll of incoming bytes on UART0 into 64-byte ring buffer
        let tick = embassy_time::Timer::after(Duration::from_millis(2));
        if let embassy_futures::select::Either::First(Ok(n)) =
            embassy_futures::select::select(uart.read(&mut rx_chunk), tick).await
        {
            for &b in &rx_chunk[..n] {
                if ring_len < rx_ring.len() {
                    rx_ring[ring_head] = b;
                    ring_head = (ring_head + 1) % rx_ring.len();
                    ring_len += 1;
                }
            }
            // Yield remainder of the 2ms slot if read resolved immediately
            embassy_time::Timer::after(Duration::from_millis(2)).await;
        }

        // 3. Feed 1 byte from the ring buffer into vm.uart_rx_in if the slot is empty
        if ring_len > 0 {
            let mut vm = SHARED_VM.lock().await;
            if vm.uart_rx_in.is_none() {
                let b = rx_ring[ring_tail];
                ring_tail = (ring_tail + 1) % rx_ring.len();
                ring_len -= 1;
                vm.uart_rx_in = Some(b);
            }
        }
    }
}

/// Asynchronous Modbus RTU RS-485 Master Engine Task
#[embassy_executor::task]
async fn modbus_master_task(
    mut uart: BufferedUart<'static, UART1>,
    mut de_re: Option<Output<'static>>,
) {
    use embedded_io_async::{Read, Write};

    info!(
        "Modbus RTU Master Task Ready on UART1 (GP8=TX, GP9=RX @ {} baud)...",
        MODBUS_BAUD_RATE
    );

    let mut tx_buf = [0u8; 256];
    let mut rx_buf = [0u8; 256];

    loop {
        // 1. Check for pending outgoing job dispatched by MicroVm
        let pending_job = {
            let mut vm = SHARED_VM.lock().await;
            vm.mb_out_job.take()
        };

        if let Some(job) = pending_job {
            let (word_snapshot, bit_snapshot) = {
                let vm = SHARED_VM.lock().await;
                (vm.word_mem, vm.bit_mem)
            };

            match uplc_vm::encode_rtu_request(&job, &word_snapshot, &bit_snapshot, &mut tx_buf) {
                Ok(tx_len) => {
                    // Set DE/RE pin HIGH (Driver Transmit mode) if configured
                    if let Some(pin) = de_re.as_mut() {
                        pin.set_high();
                        cortex_m::asm::delay(100);
                    }

                    let write_res = uart.write_all(&tx_buf[..tx_len]).await;
                    let flush_res = uart.flush().await;

                    // Allow line settle time before returning to receive mode
                    if let Some(pin) = de_re.as_mut() {
                        cortex_m::asm::delay(500);
                        pin.set_low();
                    }

                    if write_res.is_err() || flush_res.is_err() {
                        let mut vm = SHARED_VM.lock().await;
                        vm.mb_in_result = Some(uplc_vm::ModbusJobResult {
                            job_id: job.job_id,
                            instance_id: job.instance_id,
                            success: false,
                            error_code: uplc_vm::MB_ERR_TIMEOUT,
                        });
                        continue;
                    }

                    // Calculate expected response length
                    let expected_len = match job.function_code {
                        uplc_vm::MB_FC_READ_HOLDING_REGISTERS
                        | uplc_vm::MB_FC_READ_INPUT_REGISTERS => {
                            3 + (job.count as usize * 2) + 2
                        }
                        uplc_vm::MB_FC_READ_COILS | uplc_vm::MB_FC_READ_DISCRETE_INPUTS => {
                            let byte_cnt = ((job.count as usize) + 7) / 8;
                            3 + byte_cnt + 2
                        }
                        uplc_vm::MB_FC_WRITE_SINGLE_COIL
                        | uplc_vm::MB_FC_WRITE_SINGLE_REGISTER
                        | uplc_vm::MB_FC_WRITE_MULTIPLE_REGISTERS => 8,
                        _ => 8,
                    };

                    let mut rx_len = 0;
                    let timeout = embassy_time::Timer::after(Duration::from_millis(MODBUS_TIMEOUT_MS));

                    let read_future = async {
                        while rx_len < expected_len && rx_len < rx_buf.len() {
                            let n = uart.read(&mut rx_buf[rx_len..expected_len]).await?;
                            if n == 0 {
                                break;
                            }
                            rx_len += n;
                            // Check for immediate Modbus exception response (5 bytes)
                            if rx_len >= 2 && (rx_buf[1] & 0x80) != 0 && rx_len == 5 {
                                break;
                            }
                        }
                        Ok::<(), uart::Error>(())
                    };

                    match embassy_futures::select::select(read_future, timeout).await {
                        embassy_futures::select::Either::First(Ok(())) if rx_len >= 5 => {
                            let mut vm_guard = SHARED_VM.lock().await;
                            let vm = &mut *vm_guard;
                            match uplc_vm::decode_rtu_response(
                                &job,
                                &rx_buf[..rx_len],
                                &mut vm.word_mem,
                                &mut vm.bit_mem,
                            ) {
                                Ok(()) => {
                                    vm.mb_in_result = Some(uplc_vm::ModbusJobResult {
                                        job_id: job.job_id,
                                        instance_id: job.instance_id,
                                        success: true,
                                        error_code: uplc_vm::MB_ERR_OK,
                                    });
                                }
                                Err(err_code) => {
                                    vm.mb_in_result = Some(uplc_vm::ModbusJobResult {
                                        job_id: job.job_id,
                                        instance_id: job.instance_id,
                                        success: false,
                                        error_code: err_code,
                                    });
                                }
                            }
                        }
                        _ => {
                            let mut vm = SHARED_VM.lock().await;
                            vm.mb_in_result = Some(uplc_vm::ModbusJobResult {
                                job_id: job.job_id,
                                instance_id: job.instance_id,
                                success: false,
                                error_code: uplc_vm::MB_ERR_TIMEOUT,
                            });
                        }
                    }
                }
                Err(_) => {
                    let mut vm = SHARED_VM.lock().await;
                    vm.mb_in_result = Some(uplc_vm::ModbusJobResult {
                        job_id: job.job_id,
                        instance_id: job.instance_id,
                        success: false,
                        error_code: uplc_vm::MB_ERR_INVALID_PARAM,
                    });
                }
            }
        } else {
            // Non-blocking yield for 5ms when no pending jobs
            embassy_time::Timer::after(Duration::from_millis(5)).await;
        }
    }
}


