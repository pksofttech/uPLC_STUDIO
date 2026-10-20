//! Cross-platform serial port helpers for talking to the LaduinoPLC Micro-VM
//! (RP2040 USB-CDC) on Linux, Windows and macOS.
//!
//! All port enumeration, auto-detection, port-name normalisation and opening
//! logic lives here so the UI code does not need any platform-specific
//! (`/dev`, `/sys`, `stty`) handling.

use std::io::{self, Read};
use std::time::Duration;

pub use serialport::SerialPort;

/// Raspberry Pi USB vendor ID (RP2040 / Pico based devices).
pub const RP2040_VID: u16 = 0x2E8A;

/// Product-string keywords identifying a LaduinoPLC Micro-VM board.
const PLC_KEYWORDS: [&str; 4] = ["laduino", "uplc", "rp2040", "pico"];

/// Description of an enumerated serial port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortInfo {
    /// OS port name, e.g. `/dev/ttyACM0`, `COM3`, `/dev/cu.usbmodem1101`.
    pub name: String,
    /// Human readable description (USB product / manufacturer), may be empty.
    pub label: String,
    /// USB vendor ID if this is a USB serial device.
    pub vid: Option<u16>,
    /// True if the port is a USB serial device.
    pub is_usb: bool,
    /// True if the port looks like a LaduinoPLC / RP2040 Micro-VM board.
    pub is_plc: bool,
    /// True if the port is a Raspberry Pi Debugprobe (must never be auto-selected).
    pub is_debugprobe: bool,
}

impl PortInfo {
    /// Build a `PortInfo` from raw enumeration data and classify it.
    pub fn new(name: impl Into<String>, vid: Option<u16>, product: Option<&str>, is_usb: bool) -> Self {
        let name = name.into();
        let label = product.map(|p| p.trim().to_string()).unwrap_or_default();
        let lower = label.to_ascii_lowercase();
        let is_debugprobe = lower.contains("debugprobe");
        let is_plc = !is_debugprobe
            && (vid == Some(RP2040_VID) || PLC_KEYWORDS.iter().any(|k| lower.contains(k)));
        Self {
            name,
            label,
            vid,
            is_usb: is_usb || vid.is_some(),
            is_plc,
            is_debugprobe,
        }
    }

    /// Display string used in port pickers: `name (label)` or just `name`.
    pub fn display(&self) -> String {
        if self.label.is_empty() {
            self.name.clone()
        } else {
            format!("{} ({})", self.name, self.label)
        }
    }
}

/// Compare two port names (case-insensitive on Windows, where `com3 == COM3`).
pub fn same_port(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// Enumerate the serial ports currently present on the system, sorted by name.
pub fn list_ports() -> Vec<PortInfo> {
    let mut ports: Vec<PortInfo> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| keep_port_name(&p.port_name, matches!(p.port_type, serialport::SerialPortType::UsbPort(_))))
        .map(|p| match p.port_type {
            serialport::SerialPortType::UsbPort(usb) => {
                let product = usb
                    .product
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| sysfs_product(&p.port_name))
                    .or_else(|| usb.manufacturer.clone());
                PortInfo::new(p.port_name, Some(usb.vid), product.as_deref(), true)
            }
            _ => {
                let product = sysfs_product(&p.port_name);
                let is_usb = is_usb_tty_name(&p.port_name);
                PortInfo::new(p.port_name, None, product.as_deref(), is_usb)
            }
        })
        .collect();
    ports.sort_by(|a, b| a.name.cmp(&b.name));
    ports.dedup_by(|a, b| same_port(&a.name, &b.name));
    ports
}

/// Like [`list_ports`] but re-enumerates at most once per `max_age`.
///
/// Intended for per-frame UI code (e.g. connection indicators) where enumerating
/// ports every frame would be wasteful (especially on Windows / macOS).
pub fn list_ports_cached(max_age: Duration) -> Vec<PortInfo> {
    use std::sync::Mutex;
    use std::time::Instant;
    static CACHE: Mutex<Option<(Instant, Vec<PortInfo>)>> = Mutex::new(None);
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, ports)) = guard.as_ref() {
        if at.elapsed() < max_age {
            return ports.clone();
        }
    }
    let ports = list_ports();
    *guard = Some((Instant::now(), ports.clone()));
    ports
}

/// Cheap (cached) check whether the configured port (empty = auto) currently resolves
/// to a present device. Suitable for calling every UI frame.
pub fn configured_port_available(configured: &str) -> bool {
    resolve_from(configured, &list_ports_cached(Duration::from_millis(1000)), path_exists).is_some()
}

/// Filter out noisy, never-useful ports (legacy Linux `ttyS*` UARTs, macOS `tty.*` duplicates of `cu.*`).
fn keep_port_name(name: &str, is_usb: bool) -> bool {
    if cfg!(target_os = "linux") && !is_usb && name.starts_with("/dev/ttyS") {
        return false;
    }
    if cfg!(target_os = "macos") && name.starts_with("/dev/tty.") {
        return false;
    }
    true
}

fn is_usb_tty_name(name: &str) -> bool {
    name.contains("ttyACM") || name.contains("ttyUSB") || name.contains("usbmodem") || name.contains("usbserial")
}

/// Linux fallback: read the USB product string from sysfs (what the legacy code used).
#[cfg(target_os = "linux")]
fn sysfs_product(port_name: &str) -> Option<String> {
    let dev = port_name.strip_prefix("/dev/")?;
    std::fs::read_to_string(format!("/sys/class/tty/{}/device/../product", dev))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(not(target_os = "linux"))]
fn sysfs_product(_port_name: &str) -> Option<String> {
    None
}

/// Choose the best Micro-VM port from a list.
///
/// Priority 1: a port classified as PLC (RP2040 VID or matching product string), not a Debugprobe.
/// Priority 2: any USB serial port that is not a Debugprobe.
pub fn choose_plc_port(ports: &[PortInfo]) -> Option<String> {
    ports
        .iter()
        .find(|p| p.is_plc && !p.is_debugprobe)
        .or_else(|| ports.iter().find(|p| p.is_usb && !p.is_debugprobe))
        .map(|p| p.name.clone())
}

/// Auto-detect a connected LaduinoPLC Micro-VM board.
pub fn detect_plc_port() -> Option<String> {
    choose_plc_port(&list_ports())
}

/// Normalise a configured port string into an OS port name.
///
/// * strips a trailing ` (label)` suffix (e.g. `/dev/ttyACM0 (Laduino PLC)`, legacy `ttyUSB0 (Auto)`)
/// * returns `None` for an empty string / `Auto` meaning "auto-detect"
/// * on Unix, prefixes bare `tty*` / `cu.*` names with `/dev/`
pub fn normalize_port_name(configured: &str) -> Option<String> {
    let trimmed = configured.trim();
    let base = match trimmed.find(" (") {
        Some(idx) => trimmed[..idx].trim(),
        None => trimmed.split_whitespace().next().unwrap_or(""),
    };
    if base.is_empty() || base.eq_ignore_ascii_case("auto") || base.eq_ignore_ascii_case("(auto)") {
        return None;
    }
    if cfg!(unix) && (base.starts_with("tty") || base.starts_with("cu.")) {
        return Some(format!("/dev/{}", base));
    }
    Some(base.to_string())
}

/// Pure resolution logic behind [`resolve_port`], testable without hardware.
///
/// Uses the configured port if it is present (listed in `ports` or `path_exists` says so)
/// and is not a Debugprobe; otherwise falls back to [`choose_plc_port`].
/// If nothing better is found, a present-but-Debugprobe configured port is still returned.
pub fn resolve_from(
    configured: &str,
    ports: &[PortInfo],
    path_exists: impl Fn(&str) -> bool,
) -> Option<String> {
    let Some(name) = normalize_port_name(configured) else {
        return choose_plc_port(ports);
    };
    let listed = ports.iter().find(|p| same_port(&p.name, &name));
    let present = listed.is_some() || path_exists(&name);
    let is_debugprobe = listed.map(|p| p.is_debugprobe).unwrap_or(false);
    if present && !is_debugprobe {
        return Some(listed.map(|p| p.name.clone()).unwrap_or(name));
    }
    choose_plc_port(ports).or(if present { Some(name) } else { None })
}

/// Resolve the user's configured port (may be empty = auto) into an openable port name.
pub fn resolve_port(configured: &str) -> Option<String> {
    resolve_from(configured, &list_ports(), path_exists)
}

fn path_exists(name: &str) -> bool {
    cfg!(unix) && !name.is_empty() && std::path::Path::new(name).exists()
}

/// Cheap presence check for a resolved port name (used for hotplug polling).
pub fn port_present(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if cfg!(unix) {
        path_exists(name)
    } else {
        list_ports().iter().any(|p| same_port(&p.name, name))
    }
}

/// Parse a baud-rate setting string, defaulting to 115200.
pub fn parse_baud(baud: &str) -> u32 {
    baud.trim().parse::<u32>().ok().filter(|b| *b > 0).unwrap_or(115_200)
}

/// Open `port` in raw 8N1 mode without flow control, with the given read timeout, DTR asserted.
///
/// On Unix the port is opened non-exclusively (like the legacy `stty` + `open()` code) so a
/// briefly-overlapping open from the online-monitor worker does not fail with EBUSY.
/// Transient "busy" failures are retried a couple of times.
pub fn open(port: &str, baud: u32, timeout_ms: u64) -> io::Result<Box<dyn SerialPort>> {
    let builder = serialport::new(port, baud)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::None)
        .timeout(Duration::from_millis(timeout_ms))
        .dtr_on_open(true);
    #[cfg(unix)]
    let builder = builder.exclusive(false);

    let mut attempt = 0;
    loop {
        match builder.clone().open() {
            Ok(mut p) => {
                let _ = p.write_data_terminal_ready(true);
                return Ok(p);
            }
            Err(e) => {
                let err: io::Error = e.into();
                attempt += 1;
                if attempt >= 3 || err.kind() == io::ErrorKind::NotFound {
                    return Err(err);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// Read whatever bytes are available; a timeout (no data) is reported as `Ok(0)`
/// to keep the semantics of the legacy `stty min 0 time N` reads.
/// Any other error (e.g. device unplugged) is returned as `Err`.
pub fn read_some<R: Read + ?Sized>(port: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    match port.read(buf) {
        Ok(n) => Ok(n),
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(0)
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str) -> String {
        if cfg!(unix) {
            format!("/dev/{}", name)
        } else {
            name.to_string()
        }
    }

    fn sample_ports() -> Vec<PortInfo> {
        vec![
            PortInfo::new(dev("ttyACM0"), Some(RP2040_VID), Some("Debugprobe on Pico (CMSIS-DAP)"), true),
            PortInfo::new(dev("ttyACM1"), Some(RP2040_VID), Some("Laduino PLC"), true),
            PortInfo::new(dev("ttyUSB0"), Some(0x1A86), Some("USB Serial"), true),
        ]
    }

    #[test]
    fn classify_ports() {
        let p = sample_ports();
        assert!(p[0].is_debugprobe && !p[0].is_plc);
        assert!(p[1].is_plc && !p[1].is_debugprobe);
        assert!(!p[2].is_plc && p[2].is_usb);
        assert!(PortInfo::new("COM5", None, Some("uPLC Micro-VM"), false).is_plc);
        assert!(PortInfo::new("COM6", Some(RP2040_VID), None, true).is_plc);
        assert_eq!(p[1].display(), format!("{} (Laduino PLC)", dev("ttyACM1")));
        assert_eq!(PortInfo::new("COM1", None, None, false).display(), "COM1");
    }

    #[test]
    fn normalize_strips_labels_and_auto() {
        assert_eq!(normalize_port_name(""), None);
        assert_eq!(normalize_port_name("   "), None);
        assert_eq!(normalize_port_name("Auto"), None);
        assert_eq!(normalize_port_name("(Auto)"), None);
        assert_eq!(normalize_port_name("COM3"), Some("COM3".to_string()));
        assert_eq!(normalize_port_name("COM3 (USB Serial Device)"), Some("COM3".to_string()));
        assert_eq!(
            normalize_port_name("/dev/ttyACM1 (Laduino PLC)"),
            Some("/dev/ttyACM1".to_string())
        );
        assert_eq!(normalize_port_name("ttyUSB0 (Auto)"), Some(dev("ttyUSB0")));
        assert_eq!(normalize_port_name("ttyACM0"), Some(dev("ttyACM0")));
    }

    #[test]
    fn choose_prefers_plc_and_skips_debugprobe() {
        let ports = sample_ports();
        assert_eq!(choose_plc_port(&ports), Some(dev("ttyACM1")));
        // Without the PLC: fall back to any non-debugprobe USB port
        let no_plc: Vec<_> = ports.iter().filter(|p| !p.is_plc).cloned().collect();
        assert_eq!(choose_plc_port(&no_plc), Some(dev("ttyUSB0")));
        // Only a debugprobe / non-USB legacy port: nothing
        let only_dbg = vec![ports[0].clone(), PortInfo::new("COM1", None, None, false)];
        assert_eq!(choose_plc_port(&only_dbg), None);
        assert_eq!(choose_plc_port(&[]), None);
    }

    #[test]
    fn resolve_logic() {
        let ports = sample_ports();
        let none = |_: &str| false;
        // empty = auto
        assert_eq!(resolve_from("", &ports, none), Some(dev("ttyACM1")));
        // configured existing port kept (label stripped)
        assert_eq!(
            resolve_from(&format!("{} (USB Serial)", dev("ttyUSB0")), &ports, none),
            Some(dev("ttyUSB0"))
        );
        // configured debugprobe -> switch to PLC
        assert_eq!(resolve_from(&dev("ttyACM0"), &ports, none), Some(dev("ttyACM1")));
        // configured missing -> auto-detect
        assert_eq!(resolve_from("ttyACM9 (Auto)", &ports, none), Some(dev("ttyACM1")));
        // configured missing and nothing to detect
        assert_eq!(resolve_from("ttyACM9", &[], none), None);
        // configured not listed but path exists (e.g. symlink)
        assert_eq!(
            resolve_from("/dev/laduino", &[], |p: &str| p == "/dev/laduino"),
            Some("/dev/laduino".to_string())
        );
        // debugprobe only: still returned when explicitly configured
        let dbg_only = vec![ports[0].clone()];
        assert_eq!(resolve_from(&dev("ttyACM0"), &dbg_only, none), Some(dev("ttyACM0")));
    }

    #[test]
    fn resolve_strips_label_suffix() {
        // Configured value carrying a " (label)" suffix resolves to the bare port name.
        let ports = sample_ports();
        let none = |_: &str| false;
        assert_eq!(
            resolve_from(&format!("{} (Laduino PLC)", dev("ttyACM1")), &ports, none),
            Some(dev("ttyACM1"))
        );
        let com = vec![PortInfo::new("COM7", None, Some("USB Serial Device"), true)];
        assert_eq!(resolve_from("COM7 (USB Serial Device)", &com, none), Some("COM7".to_string()));
    }

    #[test]
    fn baud_parsing() {
        assert_eq!(parse_baud("57600"), 57600);
        assert_eq!(parse_baud(" 9600 "), 9600);
        assert_eq!(parse_baud("abc"), 115_200);
        assert_eq!(parse_baud("0"), 115_200);
    }

    struct MockReader(Vec<io::Result<Vec<u8>>>);
    impl Read for MockReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.0.remove(0) {
                Ok(data) => {
                    buf[..data.len()].copy_from_slice(&data);
                    Ok(data.len())
                }
                Err(e) => Err(e),
            }
        }
    }

    #[test]
    fn read_some_maps_timeout_to_zero() {
        let mut r = MockReader(vec![
            Err(io::Error::new(io::ErrorKind::TimedOut, "timeout")),
            Err(io::Error::new(io::ErrorKind::WouldBlock, "again")),
            Ok(vec![0xAA, 0x83]),
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "unplugged")),
        ]);
        let mut buf = [0u8; 8];
        assert_eq!(read_some(&mut r, &mut buf).unwrap(), 0);
        assert_eq!(read_some(&mut r, &mut buf).unwrap(), 0);
        assert_eq!(read_some(&mut r, &mut buf).unwrap(), 2);
        assert_eq!(&buf[..2], &[0xAA, 0x83]);
        assert!(read_some(&mut r, &mut buf).is_err());
    }
}
