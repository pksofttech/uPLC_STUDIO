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
use embassy_rp::peripherals::{FLASH, UART1, USB};
use embassy_rp::uart::{self, BufferedInterruptHandler, BufferedUart};
use embassy_rp::usb::{Driver as UsbDriver, InterruptHandler as UsbInterruptHandler};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State as CdcAcmState};
use embassy_usb::{Builder as UsbBuilder, Config as UsbConfig, UsbDevice};
use static_cell::StaticCell;
use uplc_vm::{
    MicroVm, CMD_DOWNLOAD_PROGRAM, CMD_MONITOR_STATUS, CMD_RUN_STOP, CMD_UPLOAD_PROGRAM,
    MAX_INSTRUCTIONS, PACKET_SOF,
};
use panic_halt as _;

mod config;
use config::*;
use portable_atomic::{AtomicBool, Ordering};

/// Global PLC Error Flag for LED and system diagnostics
static PLC_HAS_ERROR: AtomicBool = AtomicBool::new(false);

/// Global Shared Micro-VM protected by an Embassy async-friendly Mutex
static SHARED_VM: Mutex<CriticalSectionRawMutex, MicroVm> = Mutex::new(MicroVm::new());

/// Global Shared Flash handle so USB-CDC Serial can persist uploaded Bytecode
static SHARED_FLASH: Mutex<
    CriticalSectionRawMutex,
    Option<Flash<'static, FLASH, Async, FLASH_SIZE_BYTES>>,
> = Mutex::new(None);

bind_interrupts!(struct Irqs {
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
    // 3. Initialize Flash Storage & Load Saved Bytecode Program on Boot
    // -------------------------------------------------------------------------
    let mut flash = Flash::<_, Async, FLASH_SIZE_BYTES>::new(p.FLASH, p.DMA_CH0);
    load_program_from_flash(&mut flash).await;
    {
        let mut guard = SHARED_FLASH.lock().await;
        *guard = Some(flash);
    }

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

    // Spawn the Deterministic PLC Scan Cycle Task
    spawner
        .spawn(plc_scan_task(hw_io.inputs, hw_io.outputs, hw_io.status_led))
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
) {
    let mut ticker = Ticker::every(Duration::from_millis(SCAN_CYCLE_MS as u64));
    let mut scan_div: u32 = 0;
    let mut led_ms: u32 = 0;

    loop {
        ticker.next().await;
        led_ms = (led_ms + SCAN_CYCLE_MS) % 2000;

        let mut vm = SHARED_VM.lock().await;

        // 1. Sample Physical Inputs X0..X7 into Unified Bit Memory (0x00..0x07)
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

        // 2. Execute 1 PLC Scan Cycle
        vm.step_scan(SCAN_CYCLE_MS);

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
            if scan_div % 25 == 0 {
                info!(
                    "PLC Scan #{} | Status: {:?} | Prog: {} inst | Y0(GP15)={} Y1(GP14)={} | T0={}ms T1={}ms",
                    vm.scan_count,
                    plc_status,
                    vm.prog_len,
                    vm.bit_mem[0x20],
                    vm.bit_mem[0x21],
                    vm.word_mem[0xC0],
                    vm.word_mem[0xC1]
                );
            }
        }
    }
}


/// Restore saved PLC Bytecode from RP2040 Flash on power-up (or load Demo LED Blink program if empty)
async fn load_program_from_flash(flash: &mut Flash<'static, FLASH, Async, FLASH_SIZE_BYTES>) {
    let mut header = [0u8; 8];
    if flash.blocking_read(FLASH_PLC_OFFSET, &mut header).is_ok() {
        let magic = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let byte_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if magic == FLASH_MAGIC
            && byte_len > 0
            && byte_len <= MAX_INSTRUCTIONS * 4
            && byte_len % 4 == 0
        {
            static LOAD_BUF: StaticCell<[u8; MAX_INSTRUCTIONS * 4]> = StaticCell::new();
            let buf = LOAD_BUF.init([0u8; MAX_INSTRUCTIONS * 4]);
            if flash
                .blocking_read(FLASH_PLC_OFFSET + 8, &mut buf[..byte_len])
                .is_ok()
            {
                let mut vm = SHARED_VM.lock().await;
                let _ = vm.load_bytecode(&buf[..byte_len]);
                vm.running = true;
                info!(
                    "Loaded saved PLC program from Flash ({} instructions, {} bytes)",
                    byte_len / 4,
                    byte_len
                );
                return;
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

/// Erase sector and save PLC Bytecode to RP2040 Flash. Returns true if successful.
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
    page_buf[0..4].copy_from_slice(&magic_bytes);
    page_buf[4..8].copy_from_slice(&len_bytes);
    let first_chunk = payload.len().min(256 - 8);
    page_buf[8..8 + first_chunk].copy_from_slice(&payload[..first_chunk]);
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
    true
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


