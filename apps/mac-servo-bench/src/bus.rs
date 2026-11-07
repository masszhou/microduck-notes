use crate::{
    model::{IDS, Profile, Sample},
    trace::{Log, TracePort},
};
use anyhow::{Context, Result, anyhow, bail, ensure};
use rustypot::DynamixelProtocolHandler;
use serde_json::json;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

#[derive(Clone)]
pub struct Config {
    id: u8,
    eeprom: Vec<u8>,
    ram: Vec<u8>,    // registers 98..115; restore only watchdog/PWM/profile settings.
    status: Vec<u8>, // registers 64..70
}

impl Config {
    fn validate(&self, profile: Profile) -> Result<()> {
        let e = &self.eeprom;
        ensure!(
            e[11] == 3,
            "ID {} operating mode {}: run requires Position mode (3); configure in Wizard",
            self.id,
            e[11]
        );
        ensure!(
            e[10] & 0x0c == 0,
            "ID {} requires velocity-based profile and torque-on-by-goal disabled (Drive Mode bits 2,3 = 0)",
            self.id
        );
        ensure!(
            e[12] == 255,
            "ID {} secondary ID must be disabled (255)",
            self.id
        );
        ensure!(
            i32::from_le_bytes(e[20..24].try_into()?) == 0,
            "ID {}: bench expects homing offset 0",
            self.id
        );
        ensure!(
            u32::from_le_bytes(e[48..52].try_into()?) == 4095
                && u32::from_le_bytes(e[52..56].try_into()?) == 0,
            "ID {}: bench expects position limits 0..4095",
            self.id
        );
        ensure!(
            u32::from_le_bytes(e[44..48].try_into()?) >= profile.profile_velocity,
            "ID {} EEPROM velocity limit is below requested Profile Velocity {}",
            self.id,
            profile.profile_velocity
        );
        ensure!(
            self.status[4] >= 1,
            "ID {} status return level must allow READ",
            self.id
        );
        ensure!(
            self.status[6] == 0,
            "ID {} hardware error 0x{:02x}",
            self.id,
            self.status[6]
        );
        ensure!(
            self.ram[0] != 255,
            "ID {} has a latched watchdog fault; clear it in Wizard",
            self.id
        );
        ensure!(
            u16::from_le_bytes(self.ram[2..4].try_into()?) > 0,
            "ID {} Goal PWM is zero",
            self.id
        );
        Ok(())
    }
}

pub struct Bus {
    protocol: DynamixelProtocolHandler,
    port: TracePort,
    log: Log,
    saved: Vec<Config>,
    dirty: bool,
}

impl Bus {
    pub fn open(path: &str, baud: u32, log: Log) -> Result<Self> {
        #[cfg(feature = "pty-tests")]
        let baud = if path.starts_with("/dev/ttys") || path.starts_with("/dev/pts/") {
            0
        } else {
            baud
        };
        let port = serialport::new(path, baud)
            .timeout(Duration::from_millis(150))
            .open()
            .with_context(|| format!("open {path}; disconnect DYNAMIXEL Wizard from this port"))?;
        // OpenRB follows the USB CDC line-coding baud. Allow its bridge to settle.
        thread::sleep(Duration::from_millis(250));
        Ok(Self {
            protocol: DynamixelProtocolHandler::v2(),
            port: TracePort::new(port, log.clone()),
            log,
            saved: Vec::new(),
            dirty: false,
        })
    }

    fn finish<T>(
        &mut self,
        result: std::result::Result<T, Box<dyn std::error::Error>>,
    ) -> Result<T> {
        let fault = self.port.take_fault();
        let value = result.map_err(|e| anyhow!(e.to_string()))?;
        if let Some(e) = fault {
            bail!(e);
        }
        Ok(value)
    }

    fn read(&mut self, id: u8, address: u8, len: u8) -> Result<Vec<u8>> {
        let r = self.protocol.read(&mut self.port, id, address, len);
        let b = self
            .finish(r)
            .with_context(|| format!("READ ID {id} address {address}"))?;
        ensure!(b.len() == len as usize, "ID {id}: short register read");
        Ok(b)
    }

    fn read_many(&mut self, address: u8, len: u8) -> Result<Vec<Vec<u8>>> {
        let r = self.protocol.sync_read(&mut self.port, &IDS, address, len);
        let b = self
            .finish(r)
            .with_context(|| format!("SYNC_READ address {address}"))?;
        ensure!(
            b.len() == 15 && b.iter().all(|v| v.len() == len as usize),
            "incomplete SYNC_READ"
        );
        Ok(b)
    }

    fn write_many(&mut self, ids: &[u8], address: u8, data: &[Vec<u8>]) -> Result<()> {
        ensure!(
            !ids.is_empty()
                && ids.len() == data.len()
                && data
                    .iter()
                    .all(|b| !b.is_empty() && b.len() == data[0].len()),
            "invalid sync-write dimensions"
        );
        let r = self.protocol.sync_write(&mut self.port, ids, address, data);
        self.finish(r)
            .with_context(|| format!("SYNC_WRITE address {address}"))
    }

    fn same(&mut self, address: u8, data: &[u8]) -> Result<()> {
        self.write_many(&IDS, address, &vec![data.to_vec(); 15])
    }

    fn verified(&mut self, ids: &[u8], address: u8, data: &[Vec<u8>]) -> Result<()> {
        self.write_many(ids, address, data)?;
        for (&id, expected) in ids.iter().zip(data) {
            ensure!(
                self.read(id, address, expected.len() as u8)? == *expected,
                "ID {id} register {address}: read-back differs"
            );
        }
        Ok(())
    }

    pub fn probe(&mut self, stop: &AtomicBool) -> Result<()> {
        let mut errors = Vec::new();
        self.saved.clear();
        for id in IDS {
            if stop.load(Ordering::Relaxed) {
                bail!("interrupted during probe");
            }
            let result = (|| -> Result<Config> {
                let eeprom = self.read(id, 0, 64)?;
                let model = u16::from_le_bytes(eeprom[0..2].try_into()?);
                ensure!(
                    model == 1200,
                    "ID {id} model {model}, expected XL330-M288 (1200)"
                );
                ensure!(eeprom[7] == id, "ID mismatch on {id}");
                let status = self.read(id, 64, 7)?;
                let ram = self.read(id, 98, 18)?;
                self.log.emit("device", json!({"id":id,"model":model,"firmware":eeprom[6],
                    "baud_register":eeprom[8],"return_delay_us":eeprom[9] as u16*2,"drive_mode":eeprom[10],"operating_mode":eeprom[11],
                    "torque_enabled":status[0],"hardware_error":status[6],"eeprom_0_63":eeprom,"ram_98_115":ram}),
                    Some(format!("ID {id:02}: XL330-M288 firmware={} mode={} torque={} delay={}us", eeprom[6],eeprom[11],status[0],eeprom[9] as u16*2)));
                Ok(Config {
                    id,
                    eeprom,
                    status,
                    ram,
                })
            })();
            match result {
                Ok(c) => self.saved.push(c),
                Err(e) => {
                    self.log.emit(
                        "probe_error",
                        json!({"id":id,"error":format!("{e:#}")}),
                        Some(format!("ID {id:02}: {e:#}")),
                    );
                    errors.push(id);
                }
            }
        }
        ensure!(
            errors.is_empty(),
            "probe failed for IDs {errors:?}; check port, power, baud and Wizard connection"
        );
        Ok(())
    }

    pub fn sample(&mut self) -> Result<Sample> {
        Sample::decode(&self.read_many(124, 23)?)
    }

    pub fn start(&mut self, stop: &AtomicBool, profile: Profile) -> Result<[i32; 15]> {
        profile.validate()?;
        ensure!(self.saved.len() == 15, "probe must succeed before motion");
        for c in &self.saved {
            c.validate(profile)?;
        }
        self.sample()?.check()?;
        self.log.flush()?;
        ensure!(
            !stop.load(Ordering::Relaxed),
            "interrupted before enabling motion"
        );
        self.dirty = true; // Every subsequent failure (including unwind) attempts shutdown.
        self.verified(&IDS, 64, &vec![vec![0]; 15])?;
        self.verified(&IDS, 98, &vec![vec![0]; 15])?;
        let start = self.sample()?.ticks.map(|p| p.rem_euclid(4096));
        let pwm: Vec<_> = self
            .saved
            .iter()
            .map(|c| {
                u16::from_le_bytes([c.ram[2], c.ram[3]])
                    .min(200)
                    .to_le_bytes()
                    .to_vec()
            })
            .collect();
        self.verified(&IDS, 100, &pwm)?;
        self.verified(&IDS, 108, &vec![profile.bytes(); 15])?;
        self.targets(&start)?; // Hold present single-turn positions BEFORE torque enable.
        for id in IDS {
            ensure!(!stop.load(Ordering::Relaxed), "interrupted during enable");
            self.verified(&[id], 64, &[vec![1]])?;
        }
        // 50 * 20 ms = 1 s. Stops motion on loss of instructions, does NOT guarantee torque-off.
        self.verified(&IDS, 98, &vec![vec![50]; 15])?;
        self.log.emit("motion_enabled", json!({"start_ticks":start,"pwm_cap_raw":200,"profile_velocity_raw":profile.profile_velocity,"profile_acceleration_raw":profile.profile_acceleration,"watchdog_ms":1000}), Some("15 motors enabled; motion starts from measured positions; profiles read back on every ID".into()));
        Ok(start)
    }

    pub fn targets(&mut self, target: &[i32; 15]) -> Result<()> {
        ensure!(
            target.iter().all(|p| (0..=4095).contains(p)),
            "target outside 0..4095"
        );
        self.write_many(
            &IDS,
            116,
            &target
                .iter()
                .map(|p| p.to_le_bytes().to_vec())
                .collect::<Vec<_>>(),
        )
    }

    pub fn off(&mut self) -> Result<()> {
        self.dirty = true;
        self.shutdown(false)
    }

    pub fn shutdown(&mut self, restore: bool) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        self.dirty = false;
        self.log.emit(
            "shutdown_start",
            json!({}),
            Some("STOP: disabling torque on IDs 1..15".into()),
        );
        // Try all devices together first, then individually. One bad ID must not prevent the rest.
        let _ = self.same(64, &[0]);
        thread::sleep(Duration::from_millis(10));
        let mut failures = Vec::new();
        for id in IDS {
            let result = self.verified(&[id], 64, &[vec![0]]);
            if let Err(e) = result {
                failures.push(format!("ID {id} torque-off unconfirmed: {e:#}"));
                continue;
            }
            if restore {
                if let Some(c) = self.saved.iter().find(|c| c.id == id).cloned() {
                    for (address, data) in [
                        (98, c.ram[0..1].to_vec()),
                        (100, c.ram[2..4].to_vec()),
                        (108, c.ram[10..18].to_vec()),
                    ] {
                        if let Err(e) = self.verified(&[id], address, &[data]) {
                            failures.push(format!("ID {id} restore {address}: {e:#}"));
                        }
                    }
                }
            }
        }
        self.log.emit("shutdown_complete", json!({"success":failures.is_empty(),"failures":failures,"restored_ram_settings":restore}),
            Some(if failures.is_empty() { "Torque OFF confirmed for all 15 motors".into() } else { format!("STOP incomplete: {}. Disconnect external servo power if needed.", failures.join("; ")) }));
        ensure!(
            failures.is_empty(),
            "shutdown incomplete: {}",
            failures.join("; ")
        );
        Ok(())
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        if self.dirty {
            let _ = self.shutdown(true);
        }
    }
}
