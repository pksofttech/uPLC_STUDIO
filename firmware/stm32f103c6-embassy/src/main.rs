#![no_std]
#![no_main]

//! # uPLC Studio - STM32F103C6T6 Embassy Micro-VM Firmware
//!
//! Target: STM32F103C6T6 (Cortex-M3 @ 72 MHz, 32 KB Flash, 10 KB SRAM), uploaded with ST-Link V2 (SWD).
//!
//! Architecture (single Embassy thread executor, all tasks cooperative):
//! - **Task 1 (`plc_scan_task`)**: Deterministic 10ms PLC Scan Cycle (`Ticker`)
//!   1. Sample `AI0..AI3` (ADC1) and digital inputs `X0..X7` -> `vm.word_mem[0..4]` / `vm.bit_mem[0x00..0x08]`
//!   2. Run `vm.step_scan(SCAN_CYCLE_MS)`
//!   3. Drive digital outputs `Y0..Y7` <- `vm.bit_mem[0x20..0x28]` and the PC13 status LED
//! - **Task 2 (`comm_task`)**: uPLC Studio link on **USART1 (PA9 TX / PA10 RX, 115200 8N1)**
//!   `[SOF: 0xAA] [CMD: 1B] [LEN: 2B LE] [PAYLOAD] [CRC8: 1B]` — identical protocol to the RP2040 USB-CDC firmware.
//! - **Task 3 (`modbus_master_task`)**: Modbus RTU master on **USART2 (PA2 TX / PA3 RX, PA1 DE/RE)**
//!
//! The VM lives in a `ThreadModeRawMutex` blocking mutex (never disables interrupts, so the
//! interrupt-driven UART RX ring buffers keep receiving while a scan executes).

use core::cell::RefCell;
use core::mem::MaybeUninit;
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_stm32::flash::{Blocking, Flash};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::peripherals::{ADC1, USART1, USART2};
use embassy_stm32::usart::{self, BufferedUart};
use embassy_stm32::{adc, bind_interrupts};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::{Duration, Instant, Ticker, Timer};
use embedded_io_async::{Read, Write};
use panic_halt as _;
use static_cell::StaticCell;
use uplc_vm::{
    MicroVm, CMD_DOWNLOAD_PROGRAM, CMD_ENTER_BOOTSEL, CMD_ENTER_BOOTSEL_RESPONSE, CMD_FORCE_IO,
    CMD_FORCE_IO_RESPONSE, CMD_GET_IO_CONFIG, CMD_MONITOR_STATUS, CMD_PERSIST_CLEAR,
    CMD_PERSIST_CLEAR_RESPONSE, CMD_PERSIST_SAVE, CMD_PERSIST_SAVE_RESPONSE, CMD_RUN_STOP,
    CMD_SET_IO_CONFIG, CMD_STEP, CMD_UPLOAD_PROGRAM, CMD_WRITE_VALUE, CMD_WRITE_VALUE_RESPONSE,
    MAX_INSTRUCTIONS, PACKET_SOF,
};

mod config;
use config::*;

type FlashDev = Flash<'static, Blocking>;

/// Global PLC Error Flag for LED and system diagnostics
static PLC_HAS_ERROR: AtomicBool = AtomicBool::new(false);
/// CRC16 of the RETAIN region (`M64..M95`, `D96..D127`) as last saved in Flash
static RETAIN_SAVED_CRC: AtomicU16 = AtomicU16::new(0);
/// Total number of RETAIN Flash saves (`%SD7` / `D135`)
static RETAIN_SAVE_COUNT: AtomicU32 = AtomicU32::new(0);
/// Whether the RETAIN region in RAM differs from Flash (`%SM7` / `M103`)
static RETAIN_DIRTY: AtomicBool = AtomicBool::new(false);

/// Backing storage for the Micro-VM, placed in `.bss` (zero-filled by the reset handler).
///
/// A `static Mutex::new(MicroVm::new())` would put ~4.6 KB of initializer bytes into `.data`,
/// which is copied from Flash at boot — far too expensive on a 32 KB part. See [`init_vm`].
static mut VM_STORAGE: MaybeUninit<MicroVm> = MaybeUninit::zeroed();

/// Global Micro-VM handle. `ThreadModeRawMutex` = zero-cost, interrupts stay enabled.
static SHARED_VM: Mutex<ThreadModeRawMutex, RefCell<Option<&'static mut MicroVm>>> =
    Mutex::new(RefCell::new(None));

/// Turn the zero-filled `VM_STORAGE` into a valid `MicroVm` equivalent to `MicroVm::new()`.
///
/// Every `MicroVm` field is an integer / bool / array / plain struct of those (all valid as zero)
/// except the `Option<_>` fields, which are explicitly written as `None` through raw pointers
/// *before* any reference is created. Keep this list in sync with `crates/vm` `MicroVm`.
fn init_vm() {
    // SAFETY: called exactly once from `main` before any task runs; no references exist yet.
    let vm: &'static mut MicroVm = unsafe {
        let p = (*addr_of_mut!(VM_STORAGE)).as_mut_ptr();
        addr_of_mut!((*p).mb_out_job).write(None);
        addr_of_mut!((*p).mb_in_result).write(None);
        addr_of_mut!((*p).uart_tx_out).write(None);
        addr_of_mut!((*p).uart_rx_in).write(None);
        addr_of_mut!((*p).running).write(true);
        addr_of_mut!((*p).io_config).write(uplc_vm::HardwareIoConfig::default_rp2040());
        &mut *p
    };
    SHARED_VM.lock(|cell| *cell.borrow_mut() = Some(vm));
}

/// Internal Flash driver (program + RETAIN persistence)
static SHARED_FLASH: Mutex<ThreadModeRawMutex, RefCell<Option<FlashDev>>> =
    Mutex::new(RefCell::new(None));

bind_interrupts!(struct Irqs {
    USART1 => usart::BufferedInterruptHandler<USART1>;
    USART2 => usart::BufferedInterruptHandler<USART2>;
    ADC1_2 => adc::InterruptHandler<ADC1>;
});

/// Run `f` with exclusive access to the VM (must not be called re-entrantly).
#[inline]
fn with_vm<R>(f: impl FnOnce(&mut MicroVm) -> R) -> R {
    SHARED_VM.lock(|cell| f(cell.borrow_mut().as_deref_mut().unwrap()))
}

/// Run `f` with the Flash driver. Returns `None` if Flash is not initialized.
#[inline]
fn with_flash<R>(f: impl FnOnce(&mut FlashDev) -> R) -> Option<R> {
    SHARED_FLASH.lock(|cell| cell.borrow_mut().as_mut().map(f))
}

fn rcc_config() -> embassy_stm32::Config {
    use embassy_stm32::rcc::*;
    use embassy_stm32::time::Hertz;
    let mut config = embassy_stm32::Config::default();
    if USE_HSE_8MHZ {
        // 8 MHz HSE x9 = 72 MHz SYSCLK, APB1 = 36 MHz, APB2 = 72 MHz, ADC = 12 MHz
        config.rcc.hse = Some(Hse {
            freq: Hertz(8_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll = Some(Pll {
            src: PllSource::HSE,
            prediv: PllPreDiv::DIV1,
            mul: PllMul::MUL9,
        });
    } else {
        // HSI/2 (4 MHz) x16 = 64 MHz SYSCLK, APB1 = 32 MHz, ADC = 10.7 MHz
        config.rcc.pll = Some(Pll {
            src: PllSource::HSI,
            prediv: PllPreDiv::DIV2,
            mul: PllMul::MUL16,
        });
    }
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV2;
    config.rcc.apb2_pre = APBPrescaler::DIV1;
    config.rcc.adc_pre = ADCPrescaler::DIV6;
    config
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(rcc_config());
    init_vm();
    info!("uPLC Studio - STM32F103C6T6 Micro-VM | USART1 PA9/PA10 @ {} | scan {} ms", COMM_BAUD_RATE, SCAN_CYCLE_MS);

    // -------------------------------------------------------------------------
    // 1. Internal Flash: restore PLC program & RETAIN memory
    // -------------------------------------------------------------------------
    let flash = Flash::new_blocking(p.FLASH);
    SHARED_FLASH.lock(|cell| *cell.borrow_mut() = Some(flash));
    load_program_from_flash();
    {
        let mut bit_latch = [0u8; 32];
        let mut word_latch = [0i32; 32];
        if load_persistent_data_from_flash(&mut bit_latch, &mut word_latch) {
            with_vm(|vm| {
                let b = LATCH_BIT_START_ADDR as usize;
                let w = LATCH_WORD_START_ADDR as usize;
                vm.bit_mem[b..b + 32].copy_from_slice(&bit_latch);
                vm.prev_bit_mem[b..b + 32].copy_from_slice(&bit_latch);
                vm.word_mem[w..w + 32].copy_from_slice(&word_latch);
            });
        }
        RETAIN_SAVED_CRC.store(compute_retain_crc16(&bit_latch, &word_latch), Ordering::Relaxed);
        RETAIN_DIRTY.store(false, Ordering::Relaxed);
    }

    // -------------------------------------------------------------------------
    // 2. Digital I/O + ADC
    // -------------------------------------------------------------------------
    let io = configure_hardware_io!(p);
    let adc_hw = configure_hardware_adc!(p);

    // -------------------------------------------------------------------------
    // 3. USART1 (PA9 TX / PA10 RX) — uPLC Studio link
    // -------------------------------------------------------------------------
    static COMM_TX_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static COMM_RX_BUF: StaticCell<[u8; 256]> = StaticCell::new();
    let mut comm_cfg = usart::Config::default();
    comm_cfg.baudrate = COMM_BAUD_RATE;
    // Pull-up keeps RX idle-high when no USB-TTL adapter is connected (avoids 50 Hz hum framing errors)
    comm_cfg.rx_pull = embassy_stm32::gpio::Pull::Up;
    let comm_uart = BufferedUart::new(
        p.USART1,
        Irqs,
        p.PA10,
        p.PA9,
        COMM_TX_BUF.init([0; 64]),
        COMM_RX_BUF.init([0; 256]),
        comm_cfg,
    )
    .unwrap();

    // -------------------------------------------------------------------------
    // 4. USART2 (PA2 TX / PA3 RX, PA1 DE/RE) — Modbus RTU RS-485 master
    // -------------------------------------------------------------------------
    static MB_TX_BUF: StaticCell<[u8; 32]> = StaticCell::new();
    static MB_RX_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    let mut mb_cfg = usart::Config::default();
    mb_cfg.baudrate = MODBUS_BAUD_RATE;
    mb_cfg.rx_pull = embassy_stm32::gpio::Pull::Up;
    let mb_uart = BufferedUart::new(
        p.USART2,
        Irqs,
        p.PA3,
        p.PA2,
        MB_TX_BUF.init([0; 32]),
        MB_RX_BUF.init([0; 64]),
        mb_cfg,
    )
    .unwrap();
    let de_re = Output::new(p.PA1, Level::Low, Speed::Medium);

    spawner.spawn(comm_task(comm_uart)).unwrap();
    spawner.spawn(modbus_master_task(mb_uart, de_re)).unwrap();
    spawner.spawn(plc_scan_task(io, adc_hw)).unwrap();
}

// =============================================================================
// uPLC Studio Serial Link (USART1)
// =============================================================================

/// USART1 packet receiver (`SOF 0xAA | CMD | LEN | PAYLOAD | CRC8`) with inter-byte timeout
#[embassy_executor::task]
async fn comm_task(mut uart: BufferedUart<'static>) {
    static FRAME_BUF: StaticCell<[u8; MAX_INSTRUCTIONS * 4 + 8]> = StaticCell::new();
    let frame_buf = FRAME_BUF.init([0u8; MAX_INSTRUCTIONS * 4 + 8]);

    let mut rx_pos: usize = 0;
    let mut expected_len: Option<usize> = None;
    let mut chunk = [0u8; 32];

    loop {
        let n = if rx_pos == 0 {
            match uart.read(&mut chunk).await {
                Ok(n) => n,
                Err(_) => continue, // framing / overrun error -> resync on next SOF
            }
        } else {
            let timeout = Timer::after(Duration::from_millis(COMM_FRAME_TIMEOUT_MS));
            match select(uart.read(&mut chunk), timeout).await {
                Either::First(Ok(n)) => n,
                _ => {
                    // Error or silence mid-frame: drop partial frame
                    rx_pos = 0;
                    expected_len = None;
                    continue;
                }
            }
        };

        for &b in &chunk[..n] {
            if rx_pos == 0 {
                if b == PACKET_SOF {
                    frame_buf[0] = PACKET_SOF;
                    rx_pos = 1;
                    expected_len = None;
                }
                continue;
            }

            frame_buf[rx_pos] = b;
            rx_pos += 1;

            if rx_pos == 4 {
                let payload_len = u16::from_le_bytes([frame_buf[2], frame_buf[3]]) as usize;
                if payload_len <= MAX_INSTRUCTIONS * 4 {
                    expected_len = Some(5 + payload_len);
                } else {
                    rx_pos = 0;
                    expected_len = None;
                    continue;
                }
            }

            if let Some(total_len) = expected_len {
                if rx_pos == total_len {
                    handle_frame(&mut uart, frame_buf, total_len).await;
                    rx_pos = 0;
                    expected_len = None;
                }
            }
        }
    }
}

/// Snapshot the RETAIN area of the VM
fn retain_snapshot(vm: &MicroVm) -> ([u8; 32], [i32; 32]) {
    let mut b = [0u8; 32];
    let mut w = [0i32; 32];
    let bs = LATCH_BIT_START_ADDR as usize;
    let ws = LATCH_WORD_START_ADDR as usize;
    b.copy_from_slice(&vm.bit_mem[bs..bs + 32]);
    w.copy_from_slice(&vm.word_mem[ws..ws + 32]);
    (b, w)
}

/// Save RETAIN area to Flash and refresh `%SM7` / `%SD7`. Returns `true` on success.
fn save_retain_now() -> bool {
    let (b, w) = with_vm(|vm| retain_snapshot(vm));
    let ok = with_flash(|f| save_persistent_data_to_flash(f, &b, &w)).unwrap_or(false);
    if ok {
        with_vm(|vm| {
            if ENABLE_SPECIAL_MARKERS {
                vm.bit_mem[SM_RETAIN_DIRTY as usize] = 0;
            }
            if ENABLE_SPECIAL_REGISTERS {
                vm.word_mem[SD_RETAIN_SAVE_COUNT as usize] =
                    RETAIN_SAVE_COUNT.load(Ordering::Relaxed) as i32;
            }
        });
    }
    ok
}

/// Execute one complete, length-validated frame and send the response on USART1
async fn handle_frame(uart: &mut BufferedUart<'static>, buf: &mut [u8], total_len: usize) {
    let (prev_io_cfg, result) = with_vm(|vm| {
        let prev = vm.io_config;
        (prev, vm.process_serial_packet(&buf[..total_len]))
    });

    match result {
        Ok(CMD_DOWNLOAD_PROGRAM) => {
            let payload_len = total_len - 5;
            let ok = with_flash(|f| save_program_to_flash(f, &buf[4..4 + payload_len])).unwrap_or(false);
            PLC_HAS_ERROR.store(!ok, Ordering::Relaxed);
            info!("CMD 0x01: {} instructions downloaded, flash ok={}", payload_len / 4, ok);
            let resp: &[u8] = if ok { &[PACKET_SOF, 0x81, 0x00] } else { &[PACKET_SOF, 0xFF, 0x02] };
            let _ = uart.write_all(resp).await;
        }
        Ok(CMD_RUN_STOP) => {
            PLC_HAS_ERROR.store(false, Ordering::Relaxed);
            if !with_vm(|vm| vm.running) {
                let _ = save_retain_now();
            }
            let _ = uart.write_all(&[PACKET_SOF, 0x82, 0x00]).await;
        }
        Ok(CMD_MONITOR_STATUS) => {
            let len = with_vm(|vm| vm.write_status_packet(buf).unwrap_or(0));
            if len > 0 {
                let _ = uart.write_all(&buf[..len]).await;
            }
        }
        Ok(CMD_UPLOAD_PROGRAM) => {
            let len = with_vm(|vm| vm.write_program_packet(buf).unwrap_or(0));
            if len > 0 {
                let _ = uart.write_all(&buf[..len]).await;
            }
        }
        Ok(CMD_STEP) => {
            PLC_HAS_ERROR.store(false, Ordering::Relaxed);
            let _ = uart.write_all(&[PACKET_SOF, 0x85, 0x00]).await;
        }
        Ok(CMD_FORCE_IO) => {
            PLC_HAS_ERROR.store(false, Ordering::Relaxed);
            let _ = uart.write_all(&[PACKET_SOF, CMD_FORCE_IO_RESPONSE, 0x00]).await;
        }
        Ok(CMD_WRITE_VALUE) => {
            PLC_HAS_ERROR.store(false, Ordering::Relaxed);
            let _ = uart.write_all(&[PACKET_SOF, CMD_WRITE_VALUE_RESPONSE, 0x00]).await;
        }
        Ok(CMD_PERSIST_SAVE) => {
            PLC_HAS_ERROR.store(false, Ordering::Relaxed);
            let resp: &[u8] = if save_retain_now() {
                &[PACKET_SOF, CMD_PERSIST_SAVE_RESPONSE, 0x00]
            } else {
                &[PACKET_SOF, 0xFF, 0x08]
            };
            let _ = uart.write_all(resp).await;
        }
        Ok(CMD_PERSIST_CLEAR) => {
            PLC_HAS_ERROR.store(false, Ordering::Relaxed);
            let ok = with_flash(clear_persistent_data_in_flash).unwrap_or(false);
            with_vm(|vm| {
                let bs = LATCH_BIT_START_ADDR as usize;
                let ws = LATCH_WORD_START_ADDR as usize;
                vm.bit_mem[bs..bs + 32].fill(0);
                vm.word_mem[ws..ws + 32].fill(0);
                if ENABLE_SPECIAL_MARKERS {
                    vm.bit_mem[SM_RETAIN_DIRTY as usize] = 0;
                }
            });
            let resp: &[u8] = if ok {
                &[PACKET_SOF, CMD_PERSIST_CLEAR_RESPONSE, 0x00]
            } else {
                &[PACKET_SOF, 0xFF, 0x09]
            };
            let _ = uart.write_all(resp).await;
        }
        Ok(CMD_ENTER_BOOTSEL) => {
            // STM32 has no USB BOOTSEL: save RETAIN, ACK, then perform a clean system reset.
            // Firmware updates are done with ST-Link V2 (SWD) — see README.md.
            let _ = save_retain_now();
            let _ = uart.write_all(&[PACKET_SOF, CMD_ENTER_BOOTSEL_RESPONSE, 0x00]).await;
            let _ = uart.flush().await;
            Timer::after(Duration::from_millis(30)).await;
            cortex_m::peripheral::SCB::sys_reset();
        }
        Ok(CMD_GET_IO_CONFIG) => {
            // Fixed compile-time pin mapping on STM32 (see config.rs) -> not supported
            let _ = uart.write_all(&[PACKET_SOF, 0xFF, 0x0B]).await;
        }
        Ok(CMD_SET_IO_CONFIG) => {
            with_vm(|vm| vm.io_config = prev_io_cfg);
            let _ = uart.write_all(&[PACKET_SOF, 0xFF, 0x0C]).await;
        }
        Err(e) => {
            info!("Packet error: {}", e);
            PLC_HAS_ERROR.store(true, Ordering::Relaxed);
            let _ = uart.write_all(&[PACKET_SOF, 0xFF, 0x01]).await;
        }
        _ => {}
    }
    let _ = uart.flush().await;
}

// =============================================================================
// PLC Scan Cycle
// =============================================================================

/// Deterministic PLC Scan Cycle Task (`Read Inputs -> Execute Micro-VM -> Write Outputs -> Status LED`)
#[embassy_executor::task]
async fn plc_scan_task(mut io: HardwareIo, mut adc_hw: HardwareAdc) {
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

        // 1. Sample analog inputs (async ADC, done outside the VM lock)
        let mut ai = [0i32; 4];
        let mut temp_c01 = 0i32;
        if ENABLE_ANALOG_INPUTS {
            for (i, ch) in adc_hw.ai.iter_mut().enumerate() {
                ai[i] = adc_hw.adc.read(ch).await as i32;
            }
            let raw_t = adc_hw.adc.read(&mut adc_hw.temp).await as u32;
            ai[3] = raw_t as i32;
            // STM32F1 temp sensor: T = (V25 - Vsense) / Avg_Slope + 25, V25 = 1.43 V, slope = 4.3 mV/°C
            // In 0.1 °C units: 250 + (1_430_000 µV - Vsense_µV) / 430
            let vsense_uv = (raw_t * ADC_VREF_MV * 1000) / 4096;
            temp_c01 = 250 + (1_430_000 - vsense_uv as i32) / 430;
        }

        // 2. Sample digital inputs
        let raw_in: [bool; 8] = core::array::from_fn(|i| io.inputs[i].is_high());

        // 3. Execute the scan under the VM lock (no awaits inside)
        let mut outputs = [false; 8];
        let mut retain_to_save: Option<([u8; 32], [i32; 32])> = None;
        let (plc_status, heartbeat_ok) = with_vm(|vm| {
            for (i, &high) in raw_in.iter().enumerate() {
                vm.set_digital_input(i as u8, if INPUT_ACTIVE_LOW { !high } else { high });
            }
            if ENABLE_ANALOG_INPUTS {
                vm.word_mem[0..4].copy_from_slice(&ai);
                if ENABLE_SPECIAL_REGISTERS {
                    vm.word_mem[SD_CPU_TEMP_C01 as usize] = temp_c01;
                }
            }

            // No user UART on STM32F103C6 (USART1 = IDE link, USART2 = RS-485):
            // complete UART_SEND jobs immediately so the function block never hangs busy.
            if vm.uart_tx_out.take().is_some() {
                vm.uart_tx_busy = false;
            }

            if ENABLE_SPECIAL_MARKERS {
                vm.bit_mem[SM_ALWAYS_ON as usize] = 1;
                vm.bit_mem[SM_ALWAYS_OFF as usize] = 0;
                vm.bit_mem[SM_FIRST_SCAN as usize] = u8::from(vm.scan_count == 0);
                vm.bit_mem[SM_RETAIN_DIRTY as usize] = u8::from(RETAIN_DIRTY.load(Ordering::Relaxed));
                vm.bit_mem[SM_CLOCK_10MS as usize] = u8::from((total_ms % 20) < 10);
                vm.bit_mem[SM_CLOCK_100MS as usize] = u8::from((total_ms % 200) < 100);
                vm.bit_mem[SM_CLOCK_1S as usize] = u8::from((total_ms % 2000) < 1000);
                vm.bit_mem[SM_CLOCK_1MIN as usize] = u8::from((total_ms % 120_000) < 60_000);
                vm.bit_mem[SM_SYSTEM_ERROR as usize] = u8::from(PLC_HAS_ERROR.load(Ordering::Relaxed));
            }

            let scan_start = Instant::now();
            vm.step_scan(SCAN_CYCLE_MS);
            let last_scan_us = scan_start.elapsed().as_micros() as u32;
            if min_scan_us == 0 || last_scan_us < min_scan_us {
                min_scan_us = last_scan_us;
            }
            max_scan_us = max_scan_us.max(last_scan_us);

            if ENABLE_SPECIAL_MARKERS {
                vm.bit_mem[SM_SCAN_OVERRUN as usize] =
                    u8::from(last_scan_us > DEFAULT_WATCHDOG_TIMEOUT_MS * 1000);
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

            for (i, o) in outputs.iter_mut().enumerate() {
                *o = vm.get_digital_output(i as u8);
            }

            let running = vm.running && vm.prog_len > 0;
            let status = if PLC_HAS_ERROR.load(Ordering::Relaxed) {
                PlcStatus::Error
            } else if running {
                PlcStatus::Run
            } else {
                PlcStatus::Stop
            };

            if running {
                scan_div = scan_div.wrapping_add(1);
                // Check RETAIN area every 1 s (100 scans)
                if scan_div % 100 == 0 {
                    let (b, w) = retain_snapshot(vm);
                    let dirty = compute_retain_crc16(&b, &w) != RETAIN_SAVED_CRC.load(Ordering::Relaxed);
                    RETAIN_DIRTY.store(dirty, Ordering::Relaxed);
                    if ENABLE_SPECIAL_MARKERS {
                        vm.bit_mem[SM_RETAIN_DIRTY as usize] = u8::from(dirty);
                    }
                    if dirty
                        && total_ms.saturating_sub(last_retain_save_ms)
                            >= RETAIN_AUTOSAVE_MIN_INTERVAL_S * 1000
                    {
                        last_retain_save_ms = total_ms;
                        retain_to_save = Some((b, w));
                    }
                }
            }
            (status, STATUS_LED_RUN_HEARTBEAT)
        });

        // 4. Drive physical outputs Y0..Y7
        for (pin, &on) in io.outputs.iter_mut().zip(outputs.iter()) {
            pin.set_level(if on { Level::High } else { Level::Low });
        }

        // 5. Status LED: RUN = 1 Hz heartbeat / solid, STOP = 60 ms beacon every 2 s, ERROR = 5 Hz strobe
        let led_on = match plc_status {
            PlcStatus::Run => !heartbeat_ok || (led_ms % 1000) < 500,
            PlcStatus::Stop => led_ms < 60,
            PlcStatus::Error => (led_ms % 200) < 100,
        };
        io.status_led
            .set_level(if led_on != STATUS_LED_ACTIVE_LOW { Level::High } else { Level::Low });

        if total_ms % 5000 == 0 {
            let (scans, prog_len, y0) = with_vm(|vm| (vm.scan_count, vm.prog_len, vm.bit_mem[0x20]));
            info!("status={} scans={} prog={} Y0={} max_scan={}us", plc_status, scans, prog_len, y0, max_scan_us);
        }

        // 6. Periodic RETAIN auto-save (Flash page erase stalls the CPU ~20-40 ms on STM32F1)
        if let Some((b, w)) = retain_to_save {
            let _ = with_flash(|f| save_persistent_data_to_flash(f, &b, &w));
        }
    }
}

// =============================================================================
// Internal Flash Persistence (STM32F1: 1 KB pages, 16-bit programming)
// =============================================================================

/// Memory-mapped read-only view of internal Flash at `offset` (relative to `0x0800_0000`)
#[inline]
fn flash_slice(offset: u32, len: usize) -> &'static [u8] {
    // SAFETY: internal Flash is always mapped and readable; offsets are compile-time constants
    // inside the reserved PLC_STORAGE region (see memory.x).
    unsafe { core::slice::from_raw_parts((FLASH_BASE_ADDR + offset) as *const u8, len) }
}

#[inline]
fn read_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Restore saved PLC Bytecode on power-up (CRC16 verified), or load the built-in demo program
fn load_program_from_flash() {
    let header = flash_slice(FLASH_PLC_OFFSET, 12);
    let magic = read_u32(&header[0..4]);
    let byte_len = read_u32(&header[4..8]) as usize;
    let expected_crc = u16::from_le_bytes([header[8], header[9]]);

    if magic == FLASH_MAGIC && byte_len > 0 && byte_len <= MAX_INSTRUCTIONS * 4 && byte_len % 4 == 0 {
        let body = flash_slice(FLASH_PLC_OFFSET + 12, byte_len);
        if MicroVm::compute_crc16(body) == expected_crc {
            with_vm(|vm| {
                let _ = vm.load_bytecode(body);
                vm.running = true;
            });
            info!("Loaded PLC program from Flash ({} instructions)", byte_len / 4);
            return;
        }
        info!("Flash program CRC mismatch -> demo program");
    }

    // Built-in demo: blink Y0 (PB12) / Y1 (PB13) at 1 Hz in opposite phase
    const DEMO_LED_PROG: [u8; 56] = [
        0x02, 0x01, 0x00, 0x00, // 0:  LDN  X1
        0x04, 0x41, 0x00, 0x00, // 1:  ANDN M1
        0x20, 0xC0, 0xF4, 0x01, // 2:  TON  T0, K500
        0x07, 0x40, 0x00, 0x00, // 3:  ST   M0
        0x01, 0x40, 0x00, 0x00, // 4:  LD   M0
        0x20, 0xC1, 0xF4, 0x01, // 5:  TON  T1, K500
        0x07, 0x41, 0x00, 0x00, // 6:  ST   M1
        0x02, 0x01, 0x00, 0x00, // 7:  LDN  X1
        0x04, 0x40, 0x00, 0x00, // 8:  ANDN M0
        0x05, 0x00, 0x00, 0x00, // 9:  OR   X0
        0x07, 0x20, 0x00, 0x00, // 10: ST   Y0
        0x01, 0x40, 0x00, 0x00, // 11: LD   M0
        0x07, 0x21, 0x00, 0x00, // 12: ST   Y1
        0xFF, 0x00, 0x00, 0x00, // 13: END
    ];
    with_vm(|vm| {
        let _ = vm.load_bytecode(&DEMO_LED_PROG);
        vm.running = true;
    });
    info!("Loaded built-in Demo LED Blink program");
}

/// Erase pages 29..31 and save PLC Bytecode (body first, header last for power-fail safety)
fn save_program_to_flash(flash: &mut FlashDev, payload: &[u8]) -> bool {
    if payload.len() > MAX_INSTRUCTIONS * 4 || payload.len() % 4 != 0 {
        return false;
    }
    let end = FLASH_PLC_OFFSET + FLASH_PLC_PAGES * FLASH_PAGE_SIZE;
    if flash.blocking_erase(FLASH_PLC_OFFSET, end).is_err() {
        return false;
    }
    if !payload.is_empty() && flash.blocking_write(FLASH_PLC_OFFSET + 12, payload).is_err() {
        return false;
    }
    let mut header = [0u8; 12];
    header[0..4].copy_from_slice(&FLASH_MAGIC.to_le_bytes());
    header[4..8].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    header[8..10].copy_from_slice(&MicroVm::compute_crc16(payload).to_le_bytes());
    flash.blocking_write(FLASH_PLC_OFFSET, &header).is_ok()
}

/// Serialize RETAIN region: `M64..M95` (32 B) + `D96..D127` (128 B LE)
fn retain_payload(bit_latch: &[u8; 32], word_latch: &[i32; 32]) -> [u8; 160] {
    let mut data = [0u8; 160];
    data[0..32].copy_from_slice(bit_latch);
    for (i, &w) in word_latch.iter().enumerate() {
        data[32 + i * 4..36 + i * 4].copy_from_slice(&w.to_le_bytes());
    }
    data
}

fn compute_retain_crc16(bit_latch: &[u8; 32], word_latch: &[i32; 32]) -> u16 {
    MicroVm::compute_crc16(&retain_payload(bit_latch, word_latch))
}

/// Load RETAIN data from page 28. Layout: `[magic:4][version:2][crc16:2][data:160]`
fn load_persistent_data_from_flash(bit_latch: &mut [u8; 32], word_latch: &mut [i32; 32]) -> bool {
    let page = flash_slice(FLASH_PERSISTENT_OFFSET, 168);
    if read_u32(&page[0..4]) != FLASH_PERSISTENT_MAGIC {
        return false;
    }
    let version = u16::from_le_bytes([page[4], page[5]]);
    let expected_crc = u16::from_le_bytes([page[6], page[7]]);
    let data = &page[8..168];
    if version != 1 || MicroVm::compute_crc16(data) != expected_crc {
        return false;
    }
    bit_latch.copy_from_slice(&data[0..32]);
    for (i, w) in word_latch.iter_mut().enumerate() {
        *w = read_u32(&data[32 + i * 4..36 + i * 4]) as i32;
    }
    info!("Loaded RETAIN data from Flash (CRC16 0x{:04X})", expected_crc);
    true
}

/// Save RETAIN data to page 28
fn save_persistent_data_to_flash(flash: &mut FlashDev, bit_latch: &[u8; 32], word_latch: &[i32; 32]) -> bool {
    let off = FLASH_PERSISTENT_OFFSET;
    if flash.blocking_erase(off, off + FLASH_PAGE_SIZE).is_err() {
        return false;
    }
    let data = retain_payload(bit_latch, word_latch);
    let crc16 = MicroVm::compute_crc16(&data);
    let mut page = [0u8; 168];
    page[0..4].copy_from_slice(&FLASH_PERSISTENT_MAGIC.to_le_bytes());
    page[4..6].copy_from_slice(&1u16.to_le_bytes());
    page[6..8].copy_from_slice(&crc16.to_le_bytes());
    page[8..168].copy_from_slice(&data);
    if flash.blocking_write(off, &page).is_err() {
        return false;
    }
    RETAIN_SAVED_CRC.store(crc16, Ordering::Relaxed);
    RETAIN_DIRTY.store(false, Ordering::Relaxed);
    RETAIN_SAVE_COUNT.fetch_add(1, Ordering::Relaxed);
    true
}

/// Erase RETAIN page 28
fn clear_persistent_data_in_flash(flash: &mut FlashDev) -> bool {
    let off = FLASH_PERSISTENT_OFFSET;
    if flash.blocking_erase(off, off + FLASH_PAGE_SIZE).is_err() {
        return false;
    }
    RETAIN_SAVED_CRC.store(compute_retain_crc16(&[0u8; 32], &[0i32; 32]), Ordering::Relaxed);
    RETAIN_DIRTY.store(false, Ordering::Relaxed);
    true
}

// =============================================================================
// Modbus RTU Master (USART2 + PA1 DE/RE)
// =============================================================================

fn mb_result(job: &uplc_vm::ModbusMasterJob, success: bool, error_code: u16) {
    with_vm(|vm| {
        vm.mb_in_result = Some(uplc_vm::ModbusJobResult {
            job_id: job.job_id,
            instance_id: job.instance_id,
            success,
            error_code,
        });
    });
}

/// Asynchronous Modbus RTU RS-485 Master Engine Task
#[embassy_executor::task]
async fn modbus_master_task(mut uart: BufferedUart<'static>, mut de_re: Output<'static>) {
    static MB_FRAME_TX: StaticCell<[u8; 128]> = StaticCell::new();
    static MB_FRAME_RX: StaticCell<[u8; 256]> = StaticCell::new();
    let tx_buf = MB_FRAME_TX.init([0u8; 128]);
    let rx_buf = MB_FRAME_RX.init([0u8; 256]);

    loop {
        let Some(job) = with_vm(|vm| vm.mb_out_job.take()) else {
            Timer::after(Duration::from_millis(5)).await;
            continue;
        };

        let encoded = with_vm(|vm| uplc_vm::encode_rtu_request(&job, &vm.word_mem, &vm.bit_mem, tx_buf));
        let tx_len = match encoded {
            Ok(n) => n,
            Err(_) => {
                mb_result(&job, false, uplc_vm::MB_ERR_INVALID_PARAM);
                continue;
            }
        };

        // Transmit with DE/RE HIGH, release after the last stop bit (USART TC flag)
        de_re.set_high();
        let write_ok = uart.write_all(&tx_buf[..tx_len]).await.is_ok() && uart.flush().await.is_ok();
        while !embassy_stm32::pac::USART2.sr().read().tc() {}
        de_re.set_low();

        if !write_ok {
            mb_result(&job, false, uplc_vm::MB_ERR_TIMEOUT);
            continue;
        }

        let expected_len = match job.function_code {
            uplc_vm::MB_FC_READ_HOLDING_REGISTERS | uplc_vm::MB_FC_READ_INPUT_REGISTERS => {
                3 + (job.count as usize * 2) + 2
            }
            uplc_vm::MB_FC_READ_COILS | uplc_vm::MB_FC_READ_DISCRETE_INPUTS => {
                3 + (job.count as usize).div_ceil(8) + 2
            }
            _ => 8,
        }
        .min(rx_buf.len());

        let mut rx_len = 0;
        let read_future = async {
            while rx_len < expected_len {
                let n = uart.read(&mut rx_buf[rx_len..expected_len]).await?;
                if n == 0 {
                    break;
                }
                rx_len += n;
                // Modbus exception response is 5 bytes
                if rx_len == 5 && (rx_buf[1] & 0x80) != 0 {
                    break;
                }
            }
            Ok::<(), usart::Error>(())
        };
        let timeout = Timer::after(Duration::from_millis(MODBUS_TIMEOUT_MS));

        match select(read_future, timeout).await {
            Either::First(Ok(())) if rx_len >= 5 => {
                let res = with_vm(|vm| {
                    uplc_vm::decode_rtu_response(&job, &rx_buf[..rx_len], &mut vm.word_mem, &mut vm.bit_mem)
                });
                match res {
                    Ok(()) => mb_result(&job, true, uplc_vm::MB_ERR_OK),
                    Err(code) => mb_result(&job, false, code),
                }
            }
            _ => mb_result(&job, false, uplc_vm::MB_ERR_TIMEOUT),
        }
    }
}
