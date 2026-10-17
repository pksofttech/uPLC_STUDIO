#![no_std]
#![no_main]

//! # uPLC Studio - RP2040 Embassy Micro-VM Firmware
//!
//! Architecture:
//! - **Task 1 (`plc_scan_task`)**: Deterministic 10ms PLC Scan Cycle (`Ticker`)
//!   1. Sample physical GPIO inputs `X0..X7` (`GP2..GP9`) -> `vm.bit_mem[0x00..0x07]`
//!   2. Run `vm.step_scan(SCAN_CYCLE_MS)`
//!   3. Drive physical GPIO outputs `Y0..Y7` (`GP10..GP17`) <- `vm.bit_mem[0x20..0x27]`
//! - **Task 2 (`uart_comm_task`)**: Asynchronous UART0 / USB-Serial Frame Receiver
//!   - Listens for packets sent from uPLC Studio's **"⚡ Update to PLC"** menu:
//!     `[SOF: 0xAA] [CMD: 1B] [LEN: 2B LE] [PAYLOAD: 4*N B] [CRC8: 1B]`
//!   - Hot-reloads the bytecode into `SHARED_VM` and persists it to RP2040 Flash!

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::flash::{Async, Flash, ERASE_SIZE};
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::peripherals::{FLASH, UART0, USB};
use embassy_rp::uart::{self, BufferedInterruptHandler, BufferedUart};
use embassy_rp::usb::{Driver as UsbDriver, InterruptHandler as UsbInterruptHandler};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State as CdcAcmState};
use embassy_usb::{Builder as UsbBuilder, Config as UsbConfig, UsbDevice};
use static_cell::StaticCell;
use uplc_vm::{
    MicroVm, CMD_DOWNLOAD_PROGRAM, CMD_MONITOR_STATUS, CMD_RUN_STOP, MAX_INSTRUCTIONS, PACKET_SOF,
};
use panic_halt as _;

/// PLC Scan Cycle Period in milliseconds (10ms = 100 Hz deterministic scan)
const SCAN_CYCLE_MS: u32 = 10;

/// Flash offset reserved for saving PLC Bytecode (last 8KB of 2MB RP2040 Flash)
const FLASH_PLC_OFFSET: u32 = 0x1FE000;
const FLASH_SIZE_BYTES: usize = 2 * 1024 * 1024;
const FLASH_MAGIC: u32 = 0x5550_4C43; // "UPLC"

/// Global Shared Micro-VM protected by an Embassy async-friendly Mutex
static SHARED_VM: Mutex<CriticalSectionRawMutex, MicroVm> = Mutex::new(MicroVm::new());

/// Global Shared Flash handle so both USB-CDC Serial and UART0 can persist uploaded Bytecode
static SHARED_FLASH: Mutex<
    CriticalSectionRawMutex,
     Option<Flash<'static, FLASH, Async, FLASH_SIZE_BYTES>>,
> = Mutex::new(None);

bind_interrupts!(struct Irqs {
    UART0_IRQ => BufferedInterruptHandler<UART0>;
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
    info!("Demo LED Pin: GP25 <- PLC Output Y0 (%QX0.0)");
    info!("====================================================");

    // -------------------------------------------------------------------------
    // 1. Configure Hardware Digital Inputs X0..X7 (GP2..GP9)
    // -------------------------------------------------------------------------
    let inputs: [Input<'static>; 8] = [
        Input::new(p.PIN_2, Pull::Down),
        Input::new(p.PIN_3, Pull::Down),
        Input::new(p.PIN_4, Pull::Down),
        Input::new(p.PIN_5, Pull::Down),
        Input::new(p.PIN_6, Pull::Down),
        Input::new(p.PIN_7, Pull::Down),
        Input::new(p.PIN_8, Pull::Down),
        Input::new(p.PIN_9, Pull::Down),
    ];

    // -------------------------------------------------------------------------
    // 2. Configure Hardware Digital Outputs Y0..Y7 (GP10..GP17) + Demo LED (GP25)
    // -------------------------------------------------------------------------
    let outputs: [Output<'static>; 8] = [
        Output::new(p.PIN_10, Level::Low),
        Output::new(p.PIN_11, Level::Low),
        Output::new(p.PIN_12, Level::Low),
        Output::new(p.PIN_13, Level::Low),
        Output::new(p.PIN_14, Level::Low),
        Output::new(p.PIN_15, Level::Low),
        Output::new(p.PIN_16, Level::Low),
        Output::new(p.PIN_17, Level::Low),
    ];
    let demo_led = Output::new(p.PIN_25, Level::Low);

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
    // 5. Configure Buffered UART0 (GP0 = TX, GP1 = RX @ 115200 baud)
    // -------------------------------------------------------------------------
    static TX_BUF: StaticCell<[u8; 512]> = StaticCell::new();
    static RX_BUF: StaticCell<[u8; 2560]> = StaticCell::new();
    let mut uart_cfg = uart::Config::default();
    uart_cfg.baudrate = 115_200;

    let uart = BufferedUart::new(
        p.UART0,
        Irqs,
        p.PIN_0,
        p.PIN_1,
        TX_BUF.init([0; 512]),
        RX_BUF.init([0; 2560]),
        uart_cfg,
    );
    // Keep internal Pull-Up enabled on GP0 (TX) and GP1 (RX) after UART init
    embassy_rp::pac::PADS_BANK0.gpio(0).modify(|w| {
        w.set_pue(true);
        w.set_pde(false);
    });
    embassy_rp::pac::PADS_BANK0.gpio(1).modify(|w| {
        w.set_pue(true);
        w.set_pde(false);
    });

    // Spawn the UART Serial Receiver Task
    spawner.spawn(uart_comm_task(uart)).unwrap();

    // Spawn the Deterministic PLC Scan Cycle Task
    spawner
        .spawn(plc_scan_task(inputs, outputs, demo_led))
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
                                {
                                    let mut fl_guard = SHARED_FLASH.lock().await;
                                    if let Some(flash) = fl_guard.as_mut() {
                                        save_program_to_flash(flash, &frame_buf[4..4 + payload_len]);
                                    }
                                }
                                let _ = class.write_packet(&[PACKET_SOF, 0x81, 0x00]).await;
                            }
                            Ok(CMD_RUN_STOP) => {
                                info!("USB-CDC CMD 0x02: Run/Stop toggled!");
                                let _ = class.write_packet(&[PACKET_SOF, 0x82, 0x00]).await;
                            }
                            Ok(CMD_MONITOR_STATUS) => {
                                let _ = class.write_packet(&[PACKET_SOF, 0x83, 0x00, 0x01]).await;
                            }
                            Err(e) => {
                                info!("USB-CDC Packet Error: {}", e);
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

/// Deterministic PLC Scan Cycle Task (`Read Inputs -> Execute Micro-VM -> Write Outputs`)
#[embassy_executor::task]
async fn plc_scan_task(
    inputs: [Input<'static>; 8],
    mut outputs: [Output<'static>; 8],
    mut led_pin: Output<'static>,
) {
    let mut ticker = Ticker::every(Duration::from_millis(SCAN_CYCLE_MS as u64));
    let mut scan_div: u32 = 0;

    loop {
        ticker.next().await;

        let mut vm = SHARED_VM.lock().await;

        // 1. Sample Physical Inputs X0..X7 into Unified Bit Memory (0x00..0x07)
        for (i, pin) in inputs.iter().enumerate() {
            vm.set_digital_input(i as u8, pin.is_high());
        }

        // 2. Execute 1 PLC Scan Cycle
        vm.step_scan(SCAN_CYCLE_MS);

        // 3. Update Physical Outputs Y0..Y7 from Unified Bit Memory (0x20..0x27)
        for (i, out_pin) in outputs.iter_mut().enumerate() {
            if vm.get_digital_output(i as u8) {
                out_pin.set_high();
            } else {
                out_pin.set_low();
            }
        }

        // 4. Drive Onboard Demo LED Pin (GP25) directly from PLC Output Y0 (%QX0.0)
        if vm.get_digital_output(0) {
            led_pin.set_high();
        } else {
            led_pin.set_low();
        }

        if vm.running && vm.prog_len > 0 {
            scan_div = scan_div.wrapping_add(1);
            if scan_div % 25 == 0 {
                info!(
                    "PLC Scan #{} | Prog: {} inst | LED_PIN(Y0/GP25)={} LED_ALT(Y1/GP11)={} | T0={}ms T1={}ms",
                    vm.scan_count,
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

/// Asynchronous UART0 Serial Packet Receiver Task (`SOF 0xAA | CMD | LEN | PAYLOAD | CRC8`)
#[embassy_executor::task]
async fn uart_comm_task(mut uart: BufferedUart<'static, UART0>) {
    use embedded_io_async::{Read, Write};

    static FRAME_BUF_CELL: StaticCell<[u8; MAX_INSTRUCTIONS * 4 + 8]> = StaticCell::new();
    let frame_buf = FRAME_BUF_CELL.init([0u8; MAX_INSTRUCTIONS * 4 + 8]);

    info!("UART Comm Task Ready and Listening on UART0 (115200 baud)...");

    loop {
        // 1. Wait for Start-Of-Frame byte (0xAA)
        let mut sof = [0u8; 1];
        if uart.read_exact(&mut sof).await.is_err() || sof[0] != PACKET_SOF {
            continue;
        }
        frame_buf[0] = PACKET_SOF;

        // 2. Read CMD (1B) + LENGTH (2B Little-Endian)
        if uart.read_exact(&mut frame_buf[1..4]).await.is_err() {
            continue;
        }
        let payload_len = u16::from_le_bytes([frame_buf[2], frame_buf[3]]) as usize;
        if payload_len > MAX_INSTRUCTIONS * 4 {
            continue;
        }

        // 3. Read PAYLOAD + CRC8 (1B)
        let total_len = 5 + payload_len;
        if uart.read_exact(&mut frame_buf[4..total_len]).await.is_err() {
            continue;
        }

        // 4. Process Packet in Shared Micro-VM
        let result = {
            let mut vm = SHARED_VM.lock().await;
            vm.process_serial_packet(&frame_buf[..total_len])
        };

        match result {
            Ok(CMD_DOWNLOAD_PROGRAM) => {
                info!(
                    "UART CMD 0x01: Downloaded {} bytes ({} instructions) -> Saving to Flash...",
                    payload_len,
                    payload_len / 4
                );
                // Persist the newly downloaded bytecode payload to RP2040 Flash
                let payload = &frame_buf[4..4 + payload_len];
                {
                    let mut fl_guard = SHARED_FLASH.lock().await;
                    if let Some(flash) = fl_guard.as_mut() {
                        save_program_to_flash(flash, payload);
                    }
                }
                // Send ACK (0xAA, 0x81, 0x00)
                let _ = uart.write_all(&[PACKET_SOF, 0x81, 0x00]).await;
                info!("Flash saved & ACK (0xAA 0x81 0x00) sent!");
            }
            Ok(CMD_RUN_STOP) => {
                info!("UART CMD 0x02: Run/Stop toggled!");
                let _ = uart.write_all(&[PACKET_SOF, 0x82, 0x00]).await;
            }
            Ok(CMD_MONITOR_STATUS) => {
                info!("UART CMD 0x03: Sending 256-byte bit_mem status snapshot");
                let snapshot = {
                    let vm = SHARED_VM.lock().await;
                    vm.bit_mem
                };
                let _ = uart.write_all(&[PACKET_SOF, 0x83, 0x00, 0x01]).await;
                let _ = uart.write_all(&snapshot).await;
            }
            Err(e) => {
                info!("UART Packet Error: {}", e);
                let _ = uart.write_all(&[PACKET_SOF, 0xFF, 0x01]).await;
            }
            _ => {}
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
        0x07, 0x20, 0x00, 0x00, // 10: ST   Y0 (%QX0.0 LED_PIN GP25)
        0x01, 0x40, 0x00, 0x00, // 11: LD   M0
        0x07, 0x21, 0x00, 0x00, // 12: ST   Y1 (%QX0.1 LED_ALT GP11)
        0xFF, 0x00, 0x00, 0x00, // 13: END
    ];
    let mut vm = SHARED_VM.lock().await;
    let _ = vm.load_bytecode(&DEMO_LED_PROG);
    vm.running = true;
    info!("Loaded built-in Demo LED Blink program (14 instructions, 56 bytes)");
}

/// Erase sector and save PLC Bytecode to RP2040 Flash
fn save_program_to_flash(
    flash: &mut Flash<'static, FLASH, Async, FLASH_SIZE_BYTES>,
    payload: &[u8],
) {
    let _ = flash.blocking_erase(FLASH_PLC_OFFSET, FLASH_PLC_OFFSET + ERASE_SIZE as u32);
    let mut page_buf = [0xFFu8; 256];
    let magic_bytes = FLASH_MAGIC.to_le_bytes();
    let len_bytes = (payload.len() as u32).to_le_bytes();
    page_buf[0..4].copy_from_slice(&magic_bytes);
    page_buf[4..8].copy_from_slice(&len_bytes);
    let first_chunk = payload.len().min(256 - 8);
    page_buf[8..8 + first_chunk].copy_from_slice(&payload[..first_chunk]);
    let _ = flash.blocking_write(FLASH_PLC_OFFSET, &page_buf);

    let mut offset = first_chunk;
    let mut flash_addr = FLASH_PLC_OFFSET + 256;
    while offset < payload.len() {
        let mut next_page = [0xFFu8; 256];
        let chunk = (payload.len() - offset).min(256);
        next_page[..chunk].copy_from_slice(&payload[offset..offset + chunk]);
        let _ = flash.blocking_write(flash_addr, &next_page);
        offset += chunk;
        flash_addr += 256;
    }
}


