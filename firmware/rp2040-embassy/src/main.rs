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
use embassy_rp::peripherals::{FLASH, UART0};
use embassy_rp::uart::{self, BufferedInterruptHandler, BufferedUart};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};
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

bind_interrupts!(struct Irqs {
    UART0_IRQ => BufferedInterruptHandler<UART0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("====================================================");
    info!("uPLC Studio - RP2040 Micro-VM Firmware Initialized!");
    info!("Scan Cycle: {} ms | UART0 (GP0/GP1) @ 115200 baud", SCAN_CYCLE_MS);
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
    // 2. Configure Hardware Digital Outputs Y0..Y7 (GP10..GP17) + Status LED (GP25)
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
    let status_led = Output::new(p.PIN_25, Level::Low);

    // -------------------------------------------------------------------------
    // 3. Initialize Flash Storage & Load Saved Bytecode Program on Boot
    // -------------------------------------------------------------------------
    let mut flash = Flash::<_, Async, FLASH_SIZE_BYTES>::new(p.FLASH, p.DMA_CH0);
    load_program_from_flash(&mut flash).await;

    // -------------------------------------------------------------------------
    // 4. Configure Buffered UART0 (GP0 = TX, GP1 = RX @ 115200 baud)
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

    // Spawn the UART Serial Receiver & Flash Persistence Task
    spawner.spawn(uart_comm_task(uart, flash)).unwrap();

    // Spawn the Deterministic PLC Scan Cycle Task
    spawner
        .spawn(plc_scan_task(inputs, outputs, status_led))
        .unwrap();
}

/// Deterministic PLC Scan Cycle Task (`Read Inputs -> Execute Micro-VM -> Write Outputs`)
#[embassy_executor::task]
async fn plc_scan_task(
    inputs: [Input<'static>; 8],
    mut outputs: [Output<'static>; 8],
    mut status_led: Output<'static>,
) {
    let mut ticker = Ticker::every(Duration::from_millis(SCAN_CYCLE_MS as u64));
    let mut blink_div: u32 = 0;

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

        // 4. Blink Status LED when VM is in RUN mode
        if vm.running && vm.prog_len > 0 {
            blink_div = blink_div.wrapping_add(1);
            if blink_div % 25 == 0 {
                status_led.toggle();
            }
            if blink_div % 100 == 0 {
                info!(
                    "PLC Scan #{} | Prog: {} inst | X0={} X1={} -> Y0={} Y1={}",
                    vm.scan_count,
                    vm.prog_len,
                    vm.bit_mem[0],
                    vm.bit_mem[1],
                    vm.bit_mem[0x20],
                    vm.bit_mem[0x21]
                );
            }
        } else {
            status_led.set_low();
        }
    }
}

/// Asynchronous Serial Packet Receiver Task (`SOF 0xAA | CMD | LEN | PAYLOAD | CRC8`)
#[embassy_executor::task]
async fn uart_comm_task(
    mut uart: BufferedUart<'static, UART0>,
    mut flash: Flash<'static, FLASH, Async, FLASH_SIZE_BYTES>,
) {
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
                save_program_to_flash(&mut flash, payload);
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
                // Send back the 256-byte Unified Bit Memory snapshot
                let snapshot = {
                    let vm = SHARED_VM.lock().await;
                    vm.bit_mem
                };
                let _ = uart.write_all(&[PACKET_SOF, 0x83, 0x00, 0x01]).await;
                let _ = uart.write_all(&snapshot).await;
            }
            Err(e) => {
                info!("UART Packet Error: {}", e);
                // Send NACK (0xAA, 0xFF, 0x01)
                let _ = uart.write_all(&[PACKET_SOF, 0xFF, 0x01]).await;
            }
            _ => {}
        }
    }
}

/// Restore saved PLC Bytecode from RP2040 Flash on power-up (or load default program if empty)
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

    // Fallback default program on first boot: Self-holding (LD X0 / OR Y0 / ANDN X1 / ST Y0 / END)
    const DEFAULT_PROG: [u8; 20] = [
        0x01, 0x00, 0x00, 0x00, // LD   X0
        0x05, 0x20, 0x00, 0x00, // OR   Y0
        0x04, 0x01, 0x00, 0x00, // ANDN X1
        0x07, 0x20, 0x00, 0x00, // ST   Y0
        0xFF, 0x00, 0x00, 0x00, // END
    ];
    let mut vm = SHARED_VM.lock().await;
    let _ = vm.load_bytecode(&DEFAULT_PROG);
    vm.running = true;
    info!("Loaded built-in default PLC program (5 instructions, 20 bytes)");
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

