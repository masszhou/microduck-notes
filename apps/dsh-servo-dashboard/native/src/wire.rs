use anyhow::{Result, anyhow, ensure};
use rustypot::DynamixelProtocolHandler;
use serialport::{ClearBuffer, DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::{
    io::{self, Read, Write},
    time::{Duration, Instant},
};

pub trait Bus {
    fn ping(&mut self, id: u8) -> Result<bool>;
    fn read(&mut self, id: u8, address: u8, len: u8) -> Result<Vec<u8>>;
    fn write_verified(&mut self, id: u8, address: u8, data: &[u8]) -> Result<()>;
}

pub fn crc(bytes: &[u8]) -> u16 {
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
        if bytes[i..].starts_with(&[255, 255, 253, 253]) {
            out.extend([255, 255, 253]);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}
// Validate before rustypot parses or allocates from an untrusted header.
pub fn validate_frame(bytes: &[u8], id: u8, length: usize) -> Result<bool> {
    if bytes.len() < 7 {
        return Ok(false);
    }
    ensure!(bytes[..4] == [255, 255, 253, 0], "invalid status header");
    ensure!(bytes[4] == id, "unexpected status ID");
    let n = u16::from_le_bytes([bytes[5], bytes[6]]) as usize;
    ensure!((4..=256).contains(&n), "invalid status length");
    if bytes.len() < n + 7 {
        return Ok(false);
    }
    ensure!(bytes.len() == n + 7, "extra status bytes");
    ensure!(
        crc(&bytes[..n + 5]) == u16::from_le_bytes([bytes[n + 5], bytes[n + 6]]),
        "status CRC mismatch"
    );
    let body = unstuff(&bytes[7..n + 5]);
    ensure!(body.len() == length + 2, "unexpected status data length");
    ensure!(body[0] == 0x55, "not a status packet");
    ensure!(body[1] == 0, "status error/Alert 0x{:02x}", body[1]);
    Ok(true)
}

struct GuardPort {
    inner: Box<dyn SerialPort>,
    received: Vec<u8>,
    expected: Option<(u8, usize)>,
    fault: Option<String>,
    deadline: Instant,
}
impl GuardPort {
    fn begin(&mut self, id: u8, len: usize) -> Result<()> {
        self.received.clear();
        self.fault = None;
        // Unsolicited/stale replies are an ambiguous bus, not silently discarded proof.
        if self.inner.bytes_to_read()? != 0 {
            self.inner.clear(ClearBuffer::Input)?;
            return Err(anyhow!(
                "unexpected pending serial bytes; transaction refused"
            ));
        }
        self.expected = Some((id, len));
        self.deadline = Instant::now() + Duration::from_millis(120);
        Ok(())
    }
    fn finish<T>(&mut self, r: std::result::Result<T, Box<dyn std::error::Error>>) -> Result<T> {
        if let Some(e) = self.fault.take() {
            return Err(anyhow!(e));
        }
        r.map_err(|e| anyhow!(e.to_string()))
    }
}
impl Read for GuardPort {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        self.inner.set_timeout(left)?;
        let n = match self.inner.read(b) {
            Ok(n) => n,
            Err(e) => {
                if !self.received.is_empty()
                    || !matches!(
                        e.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    )
                {
                    self.fault = Some(format!("incomplete/failed status: {e}"));
                }
                return Err(e);
            }
        };
        self.received.extend_from_slice(&b[..n]);
        if let Some((id, len)) = self.expected
            && let Err(e) = validate_frame(&self.received, id, len)
        {
            self.fault = Some(e.to_string());
            return Err(io::Error::new(io::ErrorKind::InvalidData, e.to_string()));
        }
        Ok(n)
    }
}
impl Write for GuardPort {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.inner.set_timeout(Duration::from_millis(120))?;
        self.inner.write(b)
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
impl SerialPort for GuardPort {
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
            "one bus owner only",
        ))
    }
}

pub struct SerialBus {
    protocol: DynamixelProtocolHandler,
    port: GuardPort,
}
impl SerialBus {
    pub fn open(path: &str, baud: u32) -> Result<Self> {
        #[cfg(feature = "pty-tests")]
        let baud = if path.starts_with("/dev/ttys") || path.starts_with("/dev/pts/") {
            0
        } else {
            baud
        };
        let inner = serialport::new(path, baud)
            .timeout(Duration::from_millis(120))
            .open()?;
        Ok(Self {
            protocol: DynamixelProtocolHandler::v2(),
            port: GuardPort {
                inner,
                received: vec![],
                expected: None,
                fault: None,
                deadline: Instant::now(),
            },
        })
    }
}
impl Bus for SerialBus {
    fn ping(&mut self, id: u8) -> Result<bool> {
        self.port.begin(id, 3)?;
        let r = self.protocol.ping(&mut self.port, id);
        let found = self.port.finish(r)?;
        ensure!(
            found || self.port.received.is_empty(),
            "malformed ping reply"
        );
        Ok(found)
    }
    fn read(&mut self, id: u8, address: u8, len: u8) -> Result<Vec<u8>> {
        self.port.begin(id, len as usize)?;
        let r = self.protocol.read(&mut self.port, id, address, len);
        let data = self.port.finish(r)?;
        ensure!(data.len() == len as usize, "register read length mismatch");
        Ok(data)
    }
    fn write_verified(&mut self, id: u8, address: u8, data: &[u8]) -> Result<()> {
        ensure!(id <= 252, "invalid write ID");
        ensure!(
            matches!(
                (address, data.len()),
                (64, 1) | (98, 1) | (100, 2) | (108, 4) | (112, 4) | (116, 4)
            ),
            "non-allowlisted RAM write"
        );
        self.port.begin(id, 0)?;
        // Protocol 2 sync_write has wire ID254 but exactly ONE explicit target.
        // Never use broadcast WRITE or a multi-target sync_write.
        let r = self
            .protocol
            .sync_write(&mut self.port, &[id], address, &[data.to_vec()]);
        self.port.finish(r)?;
        ensure!(
            self.read(id, address, data.len() as u8)? == data,
            "ID {id} register {address} readback mismatch"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(id: u8, error: u8, data: &[u8]) -> Vec<u8> {
        let mut b = vec![255, 255, 253, 0, id, (data.len() + 4) as u8, 0, 0x55, error];
        b.extend(data);
        b.extend(crc(&b).to_le_bytes());
        b
    }
    #[test]
    fn strict_status_validation() {
        let b = frame(1, 0, &[1, 2]);
        assert!(validate_frame(&b, 1, 2).unwrap());
        assert!(!validate_frame(&b[..8], 1, 2).unwrap());
        assert!(validate_frame(&b, 2, 2).is_err());
        assert!(validate_frame(&b, 1, 3).is_err());
        assert!(validate_frame(&frame(1, 128, &[1, 2]), 1, 2).is_err());
        let mut bad = b.clone();
        bad[9] ^= 1;
        assert!(validate_frame(&bad, 1, 2).is_err());
        for n in [0, 1, 2, 3, 257, 65535u16] {
            let mut bad = b.clone();
            bad[5..7].copy_from_slice(&n.to_le_bytes());
            assert!(validate_frame(&bad[..7], 1, 2).is_err());
        }
    }
    #[test]
    fn official_crc_and_stuffing() {
        assert_eq!(crc(&[255, 255, 253, 0, 1, 3, 0, 1]), 0x4e19);
        assert_eq!(
            unstuff(&[255, 255, 253, 253, 253]),
            vec![255, 255, 253, 253]
        );
    }
}
