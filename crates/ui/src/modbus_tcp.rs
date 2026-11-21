//! # Modbus TCP Client for Factory I/O (`modbus_tcp.rs`)
//!
//! Connects the PC PLC Simulator (`PlcSimulator`) to **Factory I/O** when
//! Factory I/O is configured with the **Modbus TCP/IP Server** driver.
//!
//! Default endpoint: `127.0.0.1:1502`, Unit ID `1`.
//!
//! Synchronized I/O map:
//! - `FC 0x02` (Read Discrete Inputs, addr `0..15`)  -> `X0..X15` (`%IX0.0..%IX1.7`)
//! - `FC 0x04` (Read Input Registers, addr `0..7`)   -> `%IW0..%IW7`
//! - `FC 0x0F` (Write Multiple Coils, addr `0..15`)  <- `Y0..Y15` (`%QX0.0..%QX1.7`)
//! - `FC 0x10` (Write Multiple Registers, addr `0..7`) <- `%MW0..%MW7`

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use uplc_vm::modbus::{
    MB_FC_READ_DISCRETE_INPUTS, MB_FC_READ_INPUT_REGISTERS, MB_FC_WRITE_MULTIPLE_COILS,
    MB_FC_WRITE_MULTIPLE_REGISTERS,
};

pub const DEFAULT_FACTORY_IO_HOST: &str = "127.0.0.1";
pub const DEFAULT_FACTORY_IO_PORT: u16 = 1502;
pub const DEFAULT_FACTORY_IO_UNIT_ID: u8 = 1;

pub const FACTORY_IO_DI_COUNT: usize = 16; // X0..X15
pub const FACTORY_IO_DO_COUNT: usize = 16; // Y0..Y15
pub const FACTORY_IO_AI_COUNT: usize = 8; // %IW0..%IW7
pub const FACTORY_IO_AO_COUNT: usize = 8; // %MW0..%MW7

#[derive(Debug, Clone)]
pub struct FactoryIoConfig {
    pub host: String,
    pub port: u16,
    pub unit_id: u8,
    pub poll_interval_ms: u64,
}

impl Default for FactoryIoConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_FACTORY_IO_HOST.to_string(),
            port: DEFAULT_FACTORY_IO_PORT,
            unit_id: DEFAULT_FACTORY_IO_UNIT_ID,
            poll_interval_ms: 20,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FactoryIoShared {
    /// Inputs read from Factory I/O Discrete Inputs (`X0..X15`)
    pub discrete_inputs: [bool; FACTORY_IO_DI_COUNT],
    /// Analog inputs read from Factory I/O Input Registers (`%IW0..%IW7`)
    pub input_regs: [i32; FACTORY_IO_AI_COUNT],
    /// Outputs written to Factory I/O Coils (`Y0..Y15`)
    pub coils: [bool; FACTORY_IO_DO_COUNT],
    /// Analog outputs written to Factory I/O Holding Registers (`%MW0..%MW7`)
    pub holding_regs: [i32; FACTORY_IO_AO_COUNT],
    /// True once TCP connection is established and polling succeeds
    pub connected: bool,
    /// Human-readable status for UI
    pub status_text: String,
    /// Last transport/protocol error message, if any
    pub last_error: Option<String>,
    /// Total Modbus TCP request frames sent
    pub tx_count: u64,
    /// Total valid Modbus TCP response frames received
    pub rx_count: u64,
}

impl Default for FactoryIoShared {
    fn default() -> Self {
        Self {
            discrete_inputs: [false; FACTORY_IO_DI_COUNT],
            input_regs: [0; FACTORY_IO_AI_COUNT],
            coils: [false; FACTORY_IO_DO_COUNT],
            holding_regs: [0; FACTORY_IO_AO_COUNT],
            connected: false,
            status_text: "Disconnected".to_string(),
            last_error: None,
            tx_count: 0,
            rx_count: 0,
        }
    }
}

/// Handle managing the background Modbus TCP Client worker thread.
pub struct ModbusTcpClientHandle {
    shared: Arc<Mutex<FactoryIoShared>>,
    stop_flag: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Default for ModbusTcpClientHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl ModbusTcpClientHandle {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(FactoryIoShared::default())),
            stop_flag: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }

    pub fn shared(&self) -> &Arc<Mutex<FactoryIoShared>> {
        &self.shared
    }

    pub fn is_running(&self) -> bool {
        self.worker.as_ref().is_some_and(|h| !h.is_finished())
    }

    pub fn start(&mut self, config: FactoryIoConfig) {
        self.stop();

        self.stop_flag.store(false, Ordering::SeqCst);
        if let Ok(mut st) = self.shared.lock() {
            st.connected = false;
            st.last_error = None;
            st.status_text = format!("Connecting to {}:{}...", config.host, config.port);
        }

        let shared = Arc::clone(&self.shared);
        let stop_flag = Arc::clone(&self.stop_flag);

        let handle = thread::Builder::new()
            .name("factory-io-modbus-tcp".to_string())
            .spawn(move || worker_loop(config, shared, stop_flag))
            .ok();

        self.worker = handle;
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        if let Ok(mut st) = self.shared.lock() {
            st.connected = false;
            st.status_text = "Disconnected".to_string();
        }
    }
}

impl Drop for ModbusTcpClientHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Build a Modbus TCP ADU (`7-byte MBAP + PDU`).
pub fn build_mbap_frame(tx_id: u16, unit_id: u8, pdu: &[u8]) -> Vec<u8> {
    let mbap_len = (1 + pdu.len()) as u16;
    let mut frame = Vec::with_capacity(7 + pdu.len());
    frame.extend_from_slice(&tx_id.to_be_bytes());
    frame.extend_from_slice(&0u16.to_be_bytes()); // Protocol ID = 0 (Modbus)
    frame.extend_from_slice(&mbap_len.to_be_bytes());
    frame.push(unit_id);
    frame.extend_from_slice(pdu);
    frame
}

/// Encode `FC 0x02` (Read Discrete Inputs) or `FC 0x04` (Read Input Registers) PDU.
pub fn encode_read_pdu(fc: u8, start_addr: u16, count: u16) -> [u8; 5] {
    let [a_hi, a_lo] = start_addr.to_be_bytes();
    let [c_hi, c_lo] = count.to_be_bytes();
    [fc, a_hi, a_lo, c_hi, c_lo]
}

/// Encode `FC 0x0F` (Write Multiple Coils) PDU.
pub fn encode_write_coils_pdu(start_addr: u16, coils: &[bool]) -> Vec<u8> {
    let count = coils.len() as u16;
    let byte_count = coils.len().div_ceil(8);
    let mut pdu = Vec::with_capacity(6 + byte_count);
    pdu.push(MB_FC_WRITE_MULTIPLE_COILS);
    pdu.extend_from_slice(&start_addr.to_be_bytes());
    pdu.extend_from_slice(&count.to_be_bytes());
    pdu.push(byte_count as u8);
    let start_idx = pdu.len();
    pdu.resize(start_idx + byte_count, 0);
    for (i, &val) in coils.iter().enumerate() {
        if val {
            pdu[start_idx + i / 8] |= 1 << (i % 8);
        }
    }
    pdu
}

/// Encode `FC 0x10` (Write Multiple Registers) PDU.
pub fn encode_write_regs_pdu(start_addr: u16, regs: &[i32]) -> Vec<u8> {
    let count = regs.len() as u16;
    let byte_count = regs.len() * 2;
    let mut pdu = Vec::with_capacity(6 + byte_count);
    pdu.push(MB_FC_WRITE_MULTIPLE_REGISTERS);
    pdu.extend_from_slice(&start_addr.to_be_bytes());
    pdu.extend_from_slice(&count.to_be_bytes());
    pdu.push(byte_count as u8);
    for &val in regs {
        let raw = (val.clamp(i16::MIN as i32, i16::MAX as i32) as i16) as u16;
        pdu.extend_from_slice(&raw.to_be_bytes());
    }
    pdu
}

/// Send one Modbus TCP request and read its complete MBAP response PDU.
fn exchange_modbus_tcp(
    stream: &mut TcpStream,
    tx_id: u16,
    unit_id: u8,
    pdu: &[u8],
) -> Result<Vec<u8>, String> {
    let req = build_mbap_frame(tx_id, unit_id, pdu);
    stream
        .write_all(&req)
        .map_err(|e| format!("TCP write failed: {}", e))?;

    let mut hdr = [0u8; 7];
    stream
        .read_exact(&mut hdr)
        .map_err(|e| format!("TCP read header failed: {}", e))?;

    let resp_tx = u16::from_be_bytes([hdr[0], hdr[1]]);
    let proto_id = u16::from_be_bytes([hdr[2], hdr[3]]);
    let length = u16::from_be_bytes([hdr[4], hdr[5]]) as usize;

    if resp_tx != tx_id {
        return Err(format!("Transaction ID mismatch (expected {}, got {})", tx_id, resp_tx));
    }
    if proto_id != 0 {
        return Err(format!("Invalid Modbus TCP protocol ID: {}", proto_id));
    }
    if !(2..=260).contains(&length) {
        return Err(format!("Invalid MBAP length: {}", length));
    }

    let pdu_len = length - 1;
    let mut resp_pdu = vec![0u8; pdu_len];
    stream
        .read_exact(&mut resp_pdu)
        .map_err(|e| format!("TCP read PDU failed: {}", e))?;

    if resp_pdu[0] == (pdu[0] | 0x80) {
        let ex_code = resp_pdu.get(1).copied().unwrap_or(0);
        return Err(format!("Modbus exception 0x{:02X} for FC 0x{:02X}", ex_code, pdu[0]));
    }
    if resp_pdu[0] != pdu[0] {
        return Err(format!(
            "Function code mismatch (expected 0x{:02X}, got 0x{:02X})",
            pdu[0], resp_pdu[0]
        ));
    }

    Ok(resp_pdu)
}

fn poll_once(
    stream: &mut TcpStream,
    unit_id: u8,
    tx_id: &mut u16,
    shared: &Arc<Mutex<FactoryIoShared>>,
) -> Result<(), String> {
    let (coils_out, regs_out) = {
        let st = shared.lock().map_err(|_| "Shared state poisoned".to_string())?;
        (st.coils, st.holding_regs)
    };

    // 1. Read Discrete Inputs (X0..X15) via FC 0x02
    let mut next_tx = || {
        *tx_id = tx_id.wrapping_add(1);
        *tx_id
    };

    let di_pdu = encode_read_pdu(MB_FC_READ_DISCRETE_INPUTS, 0, FACTORY_IO_DI_COUNT as u16);
    let di_resp = exchange_modbus_tcp(stream, next_tx(), unit_id, &di_pdu)?;
    let expected_di_bytes = FACTORY_IO_DI_COUNT.div_ceil(8);
    if di_resp.len() < 2 + expected_di_bytes || di_resp[1] as usize != expected_di_bytes {
        return Err("Invalid FC 0x02 response length".to_string());
    }
    let mut new_di = [false; FACTORY_IO_DI_COUNT];
    for (i, slot) in new_di.iter_mut().enumerate() {
        *slot = ((di_resp[2 + i / 8] >> (i % 8)) & 1) != 0;
    }

    // 2. Read Input Registers (%IW0..%IW7) via FC 0x04
    let ir_pdu = encode_read_pdu(MB_FC_READ_INPUT_REGISTERS, 0, FACTORY_IO_AI_COUNT as u16);
    let ir_resp = exchange_modbus_tcp(stream, next_tx(), unit_id, &ir_pdu)?;
    let expected_ir_bytes = FACTORY_IO_AI_COUNT * 2;
    if ir_resp.len() < 2 + expected_ir_bytes || ir_resp[1] as usize != expected_ir_bytes {
        return Err("Invalid FC 0x04 response length".to_string());
    }
    let mut new_ir = [0i32; FACTORY_IO_AI_COUNT];
    for (i, slot) in new_ir.iter_mut().enumerate() {
        let hi = ir_resp[2 + i * 2];
        let lo = ir_resp[2 + i * 2 + 1];
        *slot = i16::from_be_bytes([hi, lo]) as i32;
    }

    // 3. Write Multiple Coils (Y0..Y15) via FC 0x0F
    let co_pdu = encode_write_coils_pdu(0, &coils_out);
    let _ = exchange_modbus_tcp(stream, next_tx(), unit_id, &co_pdu)?;

    // 4. Write Multiple Registers (%MW0..%MW7) via FC 0x10
    let hr_pdu = encode_write_regs_pdu(0, &regs_out);
    let _ = exchange_modbus_tcp(stream, next_tx(), unit_id, &hr_pdu)?;

    if let Ok(mut st) = shared.lock() {
        st.discrete_inputs = new_di;
        st.input_regs = new_ir;
        st.connected = true;
        st.last_error = None;
        st.tx_count = st.tx_count.saturating_add(4);
        st.rx_count = st.rx_count.saturating_add(4);
    }

    Ok(())
}

fn sleep_interruptible(total_ms: u64, stop_flag: &AtomicBool) {
    let mut remaining = total_ms;
    while remaining > 0 && !stop_flag.load(Ordering::SeqCst) {
        let step = remaining.min(25);
        thread::sleep(Duration::from_millis(step));
        remaining -= step;
    }
}

fn worker_loop(
    config: FactoryIoConfig,
    shared: Arc<Mutex<FactoryIoShared>>,
    stop_flag: Arc<AtomicBool>,
) {
    let mut tx_id: u16 = 0;
    let endpoint = format!("{}:{}", config.host.trim(), config.port);

    while !stop_flag.load(Ordering::SeqCst) {
        if let Ok(mut st) = shared.lock() {
            st.connected = false;
            st.status_text = format!("Connecting to {}...", endpoint);
        }

        let resolved = endpoint
            .to_socket_addrs()
            .map_err(|e| format!("Invalid address '{}': {}", endpoint, e))
            .and_then(|mut addrs| {
                addrs
                    .next()
                    .ok_or_else(|| format!("Could not resolve '{}'", endpoint))
            });

        let stream = match resolved {
            Ok(sock_addr) => TcpStream::connect_timeout(&sock_addr, Duration::from_millis(800))
                .map_err(|e| format!("Connect failed: {}", e)),
            Err(err) => Err(err),
        };

        match stream {
            Ok(mut stream) => {
                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
                let _ = stream.set_write_timeout(Some(Duration::from_millis(300)));

                if let Ok(mut st) = shared.lock() {
                    st.connected = true;
                    st.last_error = None;
                    st.status_text = format!("Connected to {}", endpoint);
                }

                while !stop_flag.load(Ordering::SeqCst) {
                    if let Err(err) = poll_once(&mut stream, config.unit_id, &mut tx_id, &shared) {
                        if let Ok(mut st) = shared.lock() {
                            st.connected = false;
                            st.last_error = Some(err);
                            st.status_text = format!("Reconnecting to {}...", endpoint);
                        }
                        break;
                    }
                    sleep_interruptible(config.poll_interval_ms.max(5), &stop_flag);
                }
            }
            Err(err) => {
                if let Ok(mut st) = shared.lock() {
                    st.connected = false;
                    st.last_error = Some(err);
                    st.status_text = format!("Waiting for {}...", endpoint);
                }
                sleep_interruptible(500, &stop_flag);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn test_mbap_and_pdu_encoding() {
        let pdu = encode_read_pdu(MB_FC_READ_DISCRETE_INPUTS, 0, 16);
        let frame = build_mbap_frame(0x1234, 1, &pdu);
        assert_eq!(
            frame,
            vec![0x12, 0x34, 0x00, 0x00, 0x00, 0x06, 0x01, 0x02, 0x00, 0x00, 0x00, 0x10]
        );

        let coils = [true, false, true, false, false, false, false, false, true];
        let co_pdu = encode_write_coils_pdu(0, &coils);
        assert_eq!(co_pdu, vec![0x0F, 0x00, 0x00, 0x00, 0x09, 0x02, 0x05, 0x01]);

        let regs_pdu = encode_write_regs_pdu(0, &[123, -5]);
        assert_eq!(
            regs_pdu,
            vec![0x10, 0x00, 0x00, 0x00, 0x02, 0x04, 0x00, 0x7B, 0xFF, 0xFB]
        );
    }

    #[test]
    fn test_modbus_tcp_client_loopback_poll() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().unwrap().port();

        // Spawn mock Factory I/O Modbus TCP Server
        let server_thread = thread::spawn(move || {
            let (mut conn, _) = listener.accept().expect("accept client");
            let mut captured_coils = [0u8; 2];
            let mut captured_regs = [0i32; 8];

            for _ in 0..4 {
                let mut hdr = [0u8; 7];
                conn.read_exact(&mut hdr).expect("read mbap");
                let len = u16::from_be_bytes([hdr[4], hdr[5]]) as usize;
                let mut pdu = vec![0u8; len - 1];
                conn.read_exact(&mut pdu).expect("read pdu");

                let tx_id = u16::from_be_bytes([hdr[0], hdr[1]]);
                let unit_id = hdr[6];
                let fc = pdu[0];

                let resp_pdu = match fc {
                    MB_FC_READ_DISCRETE_INPUTS => {
                        // X0=1, X2=1, X8=1 => [0x05, 0x01]
                        vec![fc, 0x02, 0x05, 0x01]
                    }
                    MB_FC_READ_INPUT_REGISTERS => {
                        // %IW0 = 1000, %IW1 = -250, rest 0
                        let mut r = vec![fc, 16];
                        r.extend_from_slice(&1000i16.to_be_bytes());
                        r.extend_from_slice(&(-250i16).to_be_bytes());
                        for _ in 2..8 {
                            r.extend_from_slice(&0i16.to_be_bytes());
                        }
                        r
                    }
                    MB_FC_WRITE_MULTIPLE_COILS => {
                        captured_coils.copy_from_slice(&pdu[6..8]);
                        vec![fc, pdu[1], pdu[2], pdu[3], pdu[4]]
                    }
                    MB_FC_WRITE_MULTIPLE_REGISTERS => {
                        for i in 0..8 {
                            let hi = pdu[6 + i * 2];
                            let lo = pdu[6 + i * 2 + 1];
                            captured_regs[i] = i16::from_be_bytes([hi, lo]) as i32;
                        }
                        vec![fc, pdu[1], pdu[2], pdu[3], pdu[4]]
                    }
                    _ => panic!("unexpected FC {}", fc),
                };

                let resp_frame = build_mbap_frame(tx_id, unit_id, &resp_pdu);
                conn.write_all(&resp_frame).expect("write response");
            }

            (captured_coils, captured_regs)
        });

        let mut client = ModbusTcpClientHandle::new();
        {
            let mut st = client.shared().lock().unwrap();
            st.coils[0] = true; // Y0
            st.coils[3] = true; // Y3
            st.holding_regs[0] = 555; // %MW0
            st.holding_regs[7] = -42; // %MW7
        }

        client.start(FactoryIoConfig {
            host: "127.0.0.1".to_string(),
            port,
            unit_id: 1,
            poll_interval_ms: 10,
        });

        let (captured_coils, captured_regs) = server_thread.join().expect("server join");
        // Wait briefly for client to process the 4th response
        for _ in 0..20 {
            if client.shared().lock().unwrap().rx_count >= 4 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        let st = client.shared().lock().unwrap().clone();
        client.stop();

        assert!(st.connected);
        assert!(st.discrete_inputs[0]); // X0
        assert!(!st.discrete_inputs[1]); // X1
        assert!(st.discrete_inputs[2]); // X2
        assert!(st.discrete_inputs[8]); // X8
        assert_eq!(st.input_regs[0], 1000); // %IW0
        assert_eq!(st.input_regs[1], -250); // %IW1
        assert_eq!(captured_coils, [0b0000_1001, 0x00]); // Y0 + Y3
        assert_eq!(captured_regs[0], 555);
        assert_eq!(captured_regs[7], -42);
    }
}
