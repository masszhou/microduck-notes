use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const DEG: f64 = 360.0 / 4096.0;
pub fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes(b[i..i + 2].try_into().unwrap())
}
fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}
fn i32_at(b: &[u8], i: usize) -> i32 {
    i32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}
#[derive(Clone)]
pub struct Snapshot {
    pub id: u8,
    pub e: Vec<u8>,
    pub r: Vec<u8>,
    pub at: Instant,
    pub seen: u64,
    pub online: bool,
    pub error: Option<String>,
}
impl Snapshot {
    pub fn new(id: u8, e: Vec<u8>, r: Vec<u8>) -> Result<Self> {
        ensure!(e.len() == 64 && r.len() == 83, "snapshot length");
        ensure!(e[7] == id, "EEPROM ID differs from response ID");
        Ok(Self {
            id,
            e,
            r,
            at: Instant::now(),
            seen: epoch_ms(),
            online: true,
            error: None,
        })
    }
    pub fn position(&self) -> i32 {
        i32_at(&self.r, 68)
    }
    pub fn goal(&self) -> i32 {
        i32_at(&self.r, 52)
    }
    pub fn raw_velocity(&self) -> i32 {
        i32_at(&self.r, 64)
    }
    pub fn holding_takeover(&self) -> Result<()> {
        if self.r[0] == 1 {
            ensure!(
                (self.goal() as f64 - self.position() as f64).abs() * DEG <= 1.0
                    && (-1..=1).contains(&self.raw_velocity()),
                "torque already on but not a near-stationary hold; takeover refused"
            );
        }
        Ok(())
    }
    pub fn model(&self) -> u16 {
        u16_at(&self.e, 0)
    }
    pub fn min(&self) -> u32 {
        u32_at(&self.e, 52)
    }
    pub fn max(&self) -> u32 {
        u32_at(&self.e, 48)
    }
    pub fn current(&self) -> i16 {
        i16::from_le_bytes(self.r[62..64].try_into().unwrap())
    }
    pub fn pwm(&self) -> u16 {
        u16_at(&self.r, 36)
    }
    pub fn acceleration(&self) -> u32 {
        u32_at(&self.r, 44)
    }
    pub fn velocity(&self) -> u32 {
        u32_at(&self.r, 48)
    }
    pub fn velocity_cap(&self) -> u32 {
        (if self.velocity() == 0 {
            5
        } else {
            self.velocity().min(5)
        })
        .min(u32_at(&self.e, 44))
    }
    pub fn telemetry_safety(&self) -> Result<()> {
        ensure!(self.r[6] == 0, "hardware error");
        ensure!(self.r[34] != 255, "latched watchdog fault");
        ensure!(
            (42..=58).contains(&u16_at(&self.r, 80)),
            "voltage outside 4.2..5.8 V"
        );
        ensure!(self.r[82] < 55, "temperature >=55 C");
        ensure!((self.current() as i32).abs() < 350, "current >=350 mA");
        Ok(())
    }
    pub fn safety(&self) -> Result<()> {
        ensure!(
            self.online && self.at.elapsed().as_millis() < 750,
            "stale/offline feedback"
        );
        ensure!(self.model() == 1200, "unsupported model (requires 1200)");
        ensure!(self.e[11] == 3, "requires operating mode 3");
        ensure!(self.e[10] & 12 == 0, "Drive Mode bits 2,3 must be clear");
        ensure!(self.e[12] == 255, "Secondary ID must be 255");
        ensure!(i32_at(&self.e, 20) == 0, "homing offset must be zero");
        ensure!(
            self.min() < self.max() && self.max() <= 4095,
            "invalid EEPROM position limits"
        );
        ensure!(
            self.position() >= self.min() as i32 && self.position() <= self.max() as i32,
            "present position outside EEPROM limits"
        );
        ensure!(
            self.r[0] <= 1 && self.r[1] <= 1,
            "invalid torque/LED boolean"
        );
        ensure!(
            (1..=2).contains(&self.r[4]),
            "Status Return Level must be 1 or 2"
        );
        self.telemetry_safety()?;
        ensure!(self.pwm() > 0 && self.pwm() <= 885, "invalid/zero Goal PWM");
        ensure!(u32_at(&self.e, 44) > 0, "invalid EEPROM velocity limit");
        Ok(())
    }
    pub fn target(&self, actor: &str, deg: f64) -> Result<i32> {
        self.safety()?;
        ensure!(deg.is_finite(), "positionDeg must be finite");
        let present = self.position() as f64 * DEG;
        let (lo, hi) = if actor == "agent" {
            ensure!((1..=3).contains(&self.id), "agent IDs restricted to 1..3");
            ensure!(
                (170.0..=190.0).contains(&present),
                "agent fresh present outside 170..190 degrees"
            );
            (
                170.0f64.max(present - 10.0).max(self.min() as f64 * DEG),
                190.0f64.min(present + 10.0).min(self.max() as f64 * DEG),
            )
        } else {
            ensure!(actor == "human", "invalid actor");
            (
                (present - 10.0).max(self.min() as f64 * DEG),
                (present + 10.0).min(self.max() as f64 * DEG),
            )
        };
        ensure!(
            (lo..=hi).contains(&deg),
            "target outside actor/EEPROM bounds [{lo},{hi}]"
        );
        let lower = (lo / DEG).ceil() as i32;
        let upper = (hi / DEG).floor() as i32;
        ensure!(lower <= upper, "no representable target");
        Ok(((deg / DEG).round() as i32).clamp(lower, upper))
    }
    pub fn json(&self, owner: Option<&str>, cleanup: bool) -> Value {
        json!({"id":self.id,"model":self.model(),"firmware":self.e[6],
            "positionTicks":self.position(),"positionDeg":self.position() as f64*DEG,
            "goalTicks":self.goal(),"goalDeg":self.goal() as f64*DEG,
            "velocityRpm":i32_at(&self.r,64) as f64*0.229,"currentMa":self.current(),
            "pwmPercent":i16::from_le_bytes(self.r[60..62].try_into().unwrap()) as f64*100.0/885.0,
            "voltage":u16_at(&self.r,80) as f64/10.0,"temperature":self.r[82],
            "torque":self.r[0]==1,"led":self.r[1]==1,"hardwareError":self.r[6],
            "operatingMode":self.e[11],"driveMode":self.e[10],"homingOffset":i32_at(&self.e,20),
            "minPositionTicks":self.min(),"maxPositionTicks":self.max(),
            "profileVelocity":self.velocity(),"profileAcceleration":self.acceleration(),"watchdog":self.r[34],
            "online":self.online,"lastSeen":self.seen,"error":self.error,
            "fresh":self.online && self.at.elapsed().as_millis()<750,"secondaryId":self.e[12],
            "statusReturnLevel":self.r[4],"goalPwm":self.pwm(),"owner":owner,"cleanupPending":cleanup})
    }
}

#[derive(Debug)]
pub enum Action {
    Ports,
    Connect { port: String, baud: u32 },
    Scan { max: u8 },
    Status,
    Heartbeat { actor: String },
    Move { id: u8, deg: f64, actor: String },
    Release { id: u8, actor: String },
    Stop { actor: String },
    Disconnect,
}
pub fn parse(v: &Value) -> Result<Action> {
    let obj = v
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("request must be object"))?;
    ensure!(
        matches!(v.get("requestId"), Some(Value::String(_)))
            || v.get("requestId").is_some_and(|x| x.is_u64() || x.is_i64()),
        "requestId must be string or integer"
    );
    ensure!(
        v["requestId"]
            .as_str()
            .is_none_or(|s| !s.is_empty() && s.len() <= 128),
        "string requestId must have1..128 bytes"
    );
    let action = v["action"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("action must be string"))?;
    let allowed: &[&str] = match action {
        "ports" | "status" | "disconnect" => &[],
        "connect" => &["port", "baud"],
        "scan" => &["maxId"],
        "heartbeat" | "stop" => &["actor"],
        "move" => &["id", "positionDeg", "actor"],
        "release" => &["id", "actor"],
        _ => anyhow::bail!("unknown action"),
    };
    ensure!(
        obj.keys()
            .all(|k| k == "requestId" || k == "action" || allowed.contains(&k.as_str())),
        "unknown request field"
    );
    let actor = || -> Result<String> {
        let a = v["actor"].as_str().unwrap_or("");
        ensure!(a == "human" || a == "agent", "actor must be human or agent");
        Ok(a.into())
    };
    let id = || -> Result<u8> {
        let n = v["id"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("id must be integer"))?;
        ensure!((0..=252).contains(&n), "id must be 0..252");
        Ok(n as u8)
    };
    Ok(match action {
        "ports" => Action::Ports,
        "status" => Action::Status,
        "disconnect" => Action::Disconnect,
        "connect" => {
            let p = v["port"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("port must be string"))?;
            ensure!(
                !p.is_empty() && p.len() <= 1024 && !p.contains('\0'),
                "invalid port"
            );
            let b = v["baud"]
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("baud must be integer"))?;
            ensure!(
                [9600, 57600, 115200, 1000000, 2000000, 3000000, 4000000].contains(&b),
                "unsupported baud"
            );
            Action::Connect {
                port: p.into(),
                baud: b as u32,
            }
        }
        "scan" => {
            let n = match v.get("maxId") {
                None => 20,
                Some(x) => x
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("maxId must be integer"))?,
            };
            ensure!(n == 20 || n == 252, "maxId must be 20 or 252");
            Action::Scan { max: n as u8 }
        }
        "heartbeat" => Action::Heartbeat { actor: actor()? },
        "stop" => Action::Stop { actor: actor()? },
        "release" => Action::Release {
            id: id()?,
            actor: actor()?,
        },
        "move" => {
            let deg = v["positionDeg"]
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("positionDeg must be finite number"))?;
            ensure!(
                deg.is_finite() && (0.0..360.0).contains(&deg),
                "positionDeg outside [0,360)"
            );
            let actor = actor()?;
            let id = id()?;
            ensure!(
                actor != "agent" || ((1..=3).contains(&id) && (170.0..=190.0).contains(&deg)),
                "agent requires ID1..3 and target170..190"
            );
            Action::Move { id, deg, actor }
        }
        _ => unreachable!(),
    })
}
