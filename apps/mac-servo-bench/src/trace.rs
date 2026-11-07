use anyhow::{Result, bail};
use serde_json::{Value, json};
use serialport::{ClearBuffer, DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Read, Write},
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Log(Arc<Mutex<LogInner>>);
struct LogInner {
    start: Instant,
    file: BufWriter<File>,
    error: Option<String>,
    quiet_bus: bool,
    seq: u64,
}

impl Log {
    pub fn new(path: &Path, quiet_bus: bool) -> Result<Self> {
        if let Some(p) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(p)?;
        }
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        Ok(Self(Arc::new(Mutex::new(LogInner {
            start: Instant::now(),
            file: BufWriter::new(file),
            error: None,
            quiet_bus,
            seq: 0,
        }))))
    }

    pub fn emit(&self, kind: &str, mut data: Value, display: Option<String>) {
        let mut l = self.0.lock().unwrap_or_else(|p| p.into_inner());
        l.seq += 1;
        let time = l.start.elapsed().as_secs_f64();
        data["event"] = kind.into();
        data["seq"] = l.seq.into();
        data["elapsed_s"] = time.into();
        if let Err(e) = writeln!(l.file, "{data}") {
            l.error.get_or_insert(e.to_string());
        }
        if let Some(line) = display.filter(|_| kind != "packet" || !l.quiet_bus) {
            if let Err(e) = writeln!(io::stdout().lock(), "[{time:9.3}] {line}") {
                l.error.get_or_insert(e.to_string());
            }
        }
    }

    pub fn flush(&self) -> Result<()> {
        let mut l = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(e) = l.file.flush() {
            l.error.get_or_insert(e.to_string());
        }
        if let Some(e) = &l.error {
            bail!("logging/output failed: {e}");
        }
        Ok(())
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Default)]
struct Frames {
    pending: Vec<u8>,
}
impl Frames {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.pending.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            if self.pending.len() < 7 {
                break;
            }
            if self.pending[..4] != [0xff, 0xff, 0xfd, 0] {
                self.pending.remove(0);
                continue;
            }
            let len = u16::from_le_bytes([self.pending[5], self.pending[6]]) as usize;
            if !(3..=4096).contains(&len) {
                self.pending.remove(0);
                continue;
            }
            if self.pending.len() < 7 + len {
                break;
            }
            out.push(self.pending.drain(..7 + len).collect());
        }
        out
    }
}

fn crc(bytes: &[u8]) -> u16 {
    let mut crc = 0u16;
    for b in bytes {
        crc ^= (*b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x8005
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn unstuff(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(&[0xff, 0xff, 0xfd, 0xfd]) {
            out.extend_from_slice(&[0xff, 0xff, 0xfd]);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

pub struct TracePort {
    inner: Box<dyn SerialPort>,
    pub log: Log,
    tx: Frames,
    rx: Frames,
    fault: Option<String>,
}
impl TracePort {
    pub fn new(inner: Box<dyn SerialPort>, log: Log) -> Self {
        Self {
            inner,
            log,
            tx: Frames::default(),
            rx: Frames::default(),
            fault: None,
        }
    }
    pub fn take_fault(&mut self) -> Option<String> {
        self.fault.take()
    }
    fn capture(&mut self, direction: &str, bytes: &[u8]) {
        self.log.emit(
            "wire_chunk",
            json!({"direction":direction,"hex":hex(bytes)}),
            None,
        );
        let frames = if direction == "TX" {
            self.tx.feed(bytes)
        } else {
            self.rx.feed(bytes)
        };
        for frame in frames {
            let n = frame.len();
            let valid = crc(&frame[..n - 2]) == u16::from_le_bytes([frame[n - 2], frame[n - 1]]);
            let body = unstuff(&frame[7..n - 2]);
            let op = body[0];
            let name = match op {
                1 => "PING",
                2 => "READ",
                3 => "WRITE",
                0x55 => "STATUS",
                0x82 => "SYNC_READ",
                0x83 => "SYNC_WRITE",
                _ => "UNKNOWN",
            };
            let mut details = json!({"direction":direction,"id":frame[4],"instruction":name,"crc_ok":valid,"hex":hex(&frame)});
            let mut summary = format!(
                "{direction} {name} id={} crc={}",
                frame[4],
                if valid { "ok" } else { "BAD" }
            );
            if op == 0x55 && body.len() >= 2 {
                details["status_error"] = body[1].into();
                details["data_hex"] = hex(&body[2..]).into();
                summary.push_str(&format!(
                    " error=0x{:02X} data={}",
                    body[1],
                    hex(&body[2..])
                ));
                // rustypot 1.6 validates CRC/ID but does not propagate status error bits.
                if direction == "RX" && valid && body[1] != 0 {
                    self.fault = Some(format!(
                        "ID {} status error/alert 0x{:02X}",
                        frame[4], body[1]
                    ));
                }
            } else if body.len() >= 5 && [2, 0x82, 0x83].contains(&op) {
                let address = u16::from_le_bytes([body[1], body[2]]);
                let width = u16::from_le_bytes([body[3], body[4]]);
                details["address"] = address.into();
                details["width"] = width.into();
                summary.push_str(&format!(" address={address} width={width}"));
                if op == 0x83 {
                    summary.push_str(" (no status reply)");
                }
            }
            summary.push_str(&format!("\n             {}", hex(&frame)));
            self.log.emit("packet", details, Some(summary));
        }
    }
}
impl Read for TracePort {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(b)?;
        self.capture("RX", &b[..n]);
        Ok(n)
    }
}
impl Write for TracePort {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(b)?;
        self.capture("TX", &b[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

macro_rules! forward {
    () => {};
    (fn $name:ident(&self $(, $arg:ident: $ty:ty)*) -> $ret:ty; $($rest:tt)*) => {
        fn $name(&self $(, $arg: $ty)*) -> $ret { self.inner.$name($($arg),*) }
        forward! { $($rest)* }
    };
    (fn $name:ident(&mut self $(, $arg:ident: $ty:ty)*) -> $ret:ty; $($rest:tt)*) => {
        fn $name(&mut self $(, $arg: $ty)*) -> $ret { self.inner.$name($($arg),*) }
        forward! { $($rest)* }
    };
}
impl SerialPort for TracePort {
    forward! {
        fn name(&self) -> Option<String>;
        fn baud_rate(&self) -> serialport::Result<u32>;
        fn data_bits(&self) -> serialport::Result<DataBits>;
        fn flow_control(&self) -> serialport::Result<FlowControl>;
        fn parity(&self) -> serialport::Result<Parity>;
        fn stop_bits(&self) -> serialport::Result<StopBits>;
        fn timeout(&self) -> Duration;
        fn set_baud_rate(&mut self, baud_rate: u32) -> serialport::Result<()>;
        fn set_data_bits(&mut self, data_bits: DataBits) -> serialport::Result<()>;
        fn set_flow_control(&mut self, flow_control: FlowControl) -> serialport::Result<()>;
        fn set_parity(&mut self, parity: Parity) -> serialport::Result<()>;
        fn set_stop_bits(&mut self, stop_bits: StopBits) -> serialport::Result<()>;
        fn set_timeout(&mut self, timeout: Duration) -> serialport::Result<()>;
        fn write_request_to_send(&mut self, level: bool) -> serialport::Result<()>;
        fn write_data_terminal_ready(&mut self, level: bool) -> serialport::Result<()>;
        fn read_clear_to_send(&mut self) -> serialport::Result<bool>;
        fn read_data_set_ready(&mut self) -> serialport::Result<bool>;
        fn read_ring_indicator(&mut self) -> serialport::Result<bool>;
        fn read_carrier_detect(&mut self) -> serialport::Result<bool>;
        fn bytes_to_read(&self) -> serialport::Result<u32>;
        fn bytes_to_write(&self) -> serialport::Result<u32>;
        fn clear(&self, buffer_to_clear: ClearBuffer) -> serialport::Result<()>;
        fn set_break(&self) -> serialport::Result<()>;
        fn clear_break(&self) -> serialport::Result<()>;
    }
    fn try_clone(&self) -> serialport::Result<Box<dyn SerialPort>> {
        Err(serialport::Error::new(
            serialport::ErrorKind::Unknown,
            "bench bus must have one owner",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_official_ping_crc_and_fragmented_stream() {
        let ping = vec![0xff, 0xff, 0xfd, 0, 1, 3, 0, 1, 0x19, 0x4e];
        assert_eq!(crc(&ping[..8]), 0x4e19);
        let mut f = Frames::default();
        assert!(f.feed(&[4, 5, 0xff]).is_empty());
        assert!(f.feed(&ping[..5]).is_empty());
        assert_eq!(f.feed(&ping[5..]), vec![ping.clone()]);
        assert_eq!(
            f.feed(&[ping.clone(), ping.clone()].concat()),
            vec![ping.clone(), ping]
        );
    }
    #[test]
    fn byte_stuffing_preserves_signed_payload_bytes() {
        assert_eq!(
            unstuff(&[0xff, 0xff, 0xfd, 0xfd, 0xfd, 0xff, 0xff, 0xfd, 0xfd]),
            vec![0xff, 0xff, 0xfd, 0xfd, 0xff, 0xff, 0xfd]
        );
    }
}
