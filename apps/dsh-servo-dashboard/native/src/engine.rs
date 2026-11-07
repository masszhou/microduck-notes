use crate::{
    model::{Action, DEG, Snapshot, epoch_ms},
    wire::{Bus, SerialBus},
};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

struct Owned {
    actor: String,
    original: Snapshot,
    lease: Instant,
    cleanup: bool,
    retry: Instant,
}
struct Scan {
    request: Value,
    next: u16,
    max: u8,
    valid: bool,
    after: Instant,
}
struct Motion {
    request: Value,
    id: u8,
    actor: String,
    requested: f64,
    target: i32,
    phase: u8,
    started: Instant,
    sent: Option<Instant>,
    baseline: Snapshot,
    command: Value,
}
pub struct Engine {
    pub bus: Option<Box<dyn Bus>>,
    pub rows: BTreeMap<u8, Snapshot>,
    owned: BTreeMap<u8, Owned>,
    scan: Option<Scan>,
    motion: Option<Motion>,
    pub outputs: Vec<Value>,
    history: VecDeque<Value>,
    pub error: Option<String>,
    port: Option<String>,
    baud: Option<u32>,
    alias_checked: Option<Instant>,
    poll_queue: VecDeque<u8>,
    next_poll: Instant,
    next_event: Instant,
    full_scan_at: Option<u64>,
}
impl Default for Engine {
    fn default() -> Self {
        Self {
            bus: None,
            rows: BTreeMap::new(),
            owned: BTreeMap::new(),
            scan: None,
            motion: None,
            outputs: vec![],
            history: VecDeque::new(),
            error: None,
            port: None,
            baud: None,
            alias_checked: None,
            poll_queue: VecDeque::new(),
            next_poll: Instant::now(),
            next_event: Instant::now(),
            full_scan_at: None,
        }
    }
}
impl Engine {
    pub fn state(&self) -> Value {
        let mut commands: Vec<_> = self.history.iter().cloned().collect();
        if let Some(m) = &self.motion {
            commands.push(m.command.clone());
        }
        json!({"connected":self.bus.is_some(),"port":self.port,"baud":self.baud,"updatedAt":epoch_ms(),
            "servos":self.rows.iter().map(|(id,s)|s.json(self.owned.get(id).map(|o|o.actor.as_str()),self.owned.get(id).is_some_and(|o|o.cleanup))).collect::<Vec<_>>(),
            "error":self.error,"commands":commands,"controlledIds":self.owned.keys().collect::<Vec<_>>(),"scanning":self.scan.is_some(),"version":env!("CARGO_PKG_VERSION"),
            "aliasCheckComplete":self.alias_checked.is_some_and(|t|t.elapsed()<Duration::from_secs(60)),
            "fullScanAt":self.full_scan_at,"fullScanValid":self.alias_checked.is_some_and(|t|t.elapsed()<Duration::from_secs(60)),
            "aliasSafetyError":if self.alias_checked.is_some_and(|t|t.elapsed()<Duration::from_secs(60)){None}else{Some("Full scan missing, older than 60 seconds, or invalidated by bus fault/new scan")}})
    }
    fn response(&mut self, request: Value, data: Value) {
        self.outputs
            .push(json!({"requestId":request,"ok":true,"data":data}));
    }
    pub fn failure(&mut self, request: Value, error: String) {
        self.outputs
            .push(json!({"requestId":request,"ok":false,"error":error}));
    }
    pub fn event(&mut self) {
        self.outputs
            .push(json!({"event":"state","data":self.state()}));
    }
    fn bus(&mut self) -> Result<&mut (dyn Bus + '_)> {
        match self.bus.as_mut() {
            Some(b) => Ok(b.as_mut()),
            None => Err(anyhow!("not connected")),
        }
    }
    fn snapshot(&mut self, id: u8) -> Result<Snapshot> {
        let e = self.bus()?.read(id, 0, 64)?;
        let r = self.bus()?.read(id, 64, 83)?;
        Snapshot::new(id, e, r)
    }
    fn refresh(&mut self, id: u8) -> Result<()> {
        let r = self.bus()?.read(id, 64, 83)?;
        let e = self
            .rows
            .get(&id)
            .ok_or_else(|| anyhow!("unknown ID"))?
            .e
            .clone();
        let s = Snapshot::new(id, e, r)?;
        self.rows.insert(id, s);
        Ok(())
    }
    pub fn handle(&mut self, request: Value, action: Action) {
        if let Err(e) = self.handle_inner(request.clone(), action) {
            self.failure(request, format!("{e:#}"));
        }
    }
    fn handle_inner(&mut self, request: Value, action: Action) -> Result<()> {
        match action {
            Action::Ports => {
                let ports=serialport::available_ports()?.into_iter().map(|p|match p.port_type {
                    serialport::SerialPortType::UsbPort(u)=>json!({"path":p.port_name,"manufacturer":u.manufacturer,"product":u.product,"serialNumber":u.serial_number,"vid":u.vid,"pid":u.pid}),
                    _=>json!({"path":p.port_name,"manufacturer":null,"product":null,"serialNumber":null,"vid":null,"pid":null})
                }).collect::<Vec<_>>();
                self.response(request, json!({"ports":ports}));
            }
            Action::Connect { port, baud } => {
                ensure!(self.bus.is_none(), "already connected; disconnect first");
                self.bus = Some(Box::new(SerialBus::open(&port, baud)?));
                self.port = Some(port);
                self.baud = Some(baud);
                self.rows.clear();
                self.error = None;
                self.alias_checked = None;
                self.scan = Some(Scan {
                    request,
                    next: 1,
                    max: 20,
                    valid: true,
                    after: Instant::now() + Duration::from_millis(250),
                });
                self.event();
            }
            Action::Scan { max } => {
                ensure!(self.bus.is_some(), "not connected");
                ensure!(
                    self.owned.is_empty(),
                    "release all touched motors before scan"
                );
                ensure!(self.scan.is_none(), "scan already running");
                self.alias_checked = None;
                self.error = None;
                self.scan = Some(Scan {
                    request,
                    next: if max == 252 { 0 } else { 1 },
                    max,
                    valid: true,
                    after: Instant::now(),
                });
                self.event();
            }
            Action::Status => self.response(request, self.state()),
            Action::Heartbeat { actor } => {
                // Only an explicit actor heartbeat renews existing ownership.
                for o in self
                    .owned
                    .values_mut()
                    .filter(|o| o.actor == actor && !o.cleanup)
                {
                    if o.lease.elapsed() < Duration::from_secs(5) {
                        o.lease = Instant::now();
                    } else {
                        o.cleanup = true;
                        o.retry = Instant::now();
                    }
                }
                if self
                    .motion
                    .as_ref()
                    .is_some_and(|m| self.owned.get(&m.id).is_some_and(|o| o.cleanup))
                {
                    self.cancel_if(
                        None,
                        Some(&actor),
                        "actor heartbeat arrived after lease expiry",
                    );
                }
                self.response(request, self.state());
            }
            Action::Move { id, deg, actor } => {
                ensure!(self.scan.is_none() && self.motion.is_none(), "worker busy");
                ensure!(self.rows.contains_key(&id), "ID not discovered");
                ensure!(
                    self.owned
                        .get(&id)
                        .is_none_or(|o| o.actor == actor && !o.cleanup),
                    "ID owned by another actor or cleanup pending"
                );
                ensure!(
                    self.alias_checked
                        .is_some_and(|t| t.elapsed() < Duration::from_secs(60)),
                    "alias safety unknown/expired: run scan maxId252 while released"
                );
                ensure!(
                    self.rows
                        .values()
                        .all(|s| s.online && s.model() == 1200 && s.e[12] == 255),
                    "unknown/offline device or SecondaryID alias risk"
                );
                for s in self.rows.values() {
                    s.telemetry_safety()?;
                }
                let s = match self.snapshot(id) {
                    Ok(s) => s,
                    Err(e) => {
                        let reason = format!("ID {id} preflight read failed: {e:#}");
                        if let Some(s) = self.rows.get_mut(&id) {
                            s.online = false;
                            s.error = Some(reason.clone());
                        }
                        self.hazard(reason.clone());
                        return Err(anyhow!(reason));
                    }
                };
                let target = s.target(&actor, deg)?;
                s.holding_takeover()?;
                self.rows.insert(id, s.clone());
                // Dirty ownership is recorded BEFORE the first possible write.
                self.owned.entry(id).or_insert_with(|| Owned {
                    actor: actor.clone(),
                    original: s.clone(),
                    lease: Instant::now(),
                    cleanup: false,
                    retry: Instant::now(),
                });
                let command = json!({"requestId":request,"id":id,"actor":actor,"positionDeg":deg,"targetTicks":target,"targetDeg":target as f64*DEG,
                    "accepted":true,"readbackVerified":false,"arrivalVerified":false,"unknown":false,"status":"accepted","outcome":"pending","error":null,"acceptedAt":epoch_ms(),"completedAt":null});
                self.motion = Some(Motion {
                    request,
                    id,
                    actor,
                    requested: deg,
                    target,
                    phase: 0,
                    started: Instant::now(),
                    sent: None,
                    baseline: s,
                    command,
                });
                self.event();
            }
            Action::Release { id, actor } => {
                if let Some(o) = self.owned.get(&id) {
                    ensure!(o.actor == actor, "actor does not own ID");
                } else {
                    ensure!(
                        actor == "human",
                        "agent release requires an already touched matching-owner ID"
                    );
                    ensure!(self.scan.is_none(), "scan in progress");
                    ensure!(self.rows.contains_key(&id), "ID not discovered");
                    ensure!(
                        self.alias_checked
                            .is_some_and(|t| t.elapsed() < Duration::from_secs(60)),
                        "alias safety unknown/expired: run scan maxId252 before taking release ownership"
                    );
                    ensure!(
                        self.rows
                            .values()
                            .all(|s| s.online && s.model() == 1200 && s.e[12] == 255),
                        "unknown/offline device or SecondaryID alias risk"
                    );
                    let s = match self.snapshot(id) {
                        Ok(s) => s,
                        Err(e) => {
                            let reason = format!("ID {id} release preflight read failed: {e:#}");
                            if let Some(s) = self.rows.get_mut(&id) {
                                s.online = false;
                                s.error = Some(reason.clone());
                            }
                            self.hazard(reason.clone());
                            return Err(anyhow!(reason));
                        }
                    };
                    s.safety()?;
                    // Explicit human release may acquire this ID solely for safe cleanup.
                    // No holding-takeover check or goal write: unloading is the requested action.
                    self.rows.insert(id, s.clone());
                    self.owned.insert(
                        id,
                        Owned {
                            actor: actor.clone(),
                            original: s,
                            lease: Instant::now(),
                            cleanup: true,
                            retry: Instant::now(),
                        },
                    );
                    self.event();
                }
                self.cancel_if(Some(id), Some(&actor), "released");
                self.mark_cleanup(Some(id), Some(&actor));
                self.cleanup_id(id);
                self.response(request, self.state());
            }
            Action::Stop { actor } => {
                let owner = if actor == "human" {
                    None
                } else {
                    Some(actor.as_str())
                };
                self.cancel_if(None, owner, "stopped");
                self.mark_cleanup(None, owner);
                self.cleanup_one();
                self.response(request, self.state());
            }
            Action::Disconnect => {
                self.cancel_scan("disconnected");
                self.cancel_if(None, None, "disconnected");
                self.mark_cleanup(None, None);
                let ids: Vec<_> = self.owned.keys().copied().collect();
                for id in ids {
                    self.cleanup_id(id);
                }
                ensure!(
                    self.owned.is_empty(),
                    "cleanup incomplete; connection retained for retry; disconnect not complete"
                );
                self.bus = None;
                self.port = None;
                self.baud = None;
                self.alias_checked = None;
                for s in self.rows.values_mut() {
                    s.online = false;
                    s.error = Some("disconnected".into());
                }
                self.response(request, self.state());
            }
        }
        Ok(())
    }
    fn cancel_scan(&mut self, reason: &str) {
        if let Some(s) = self.scan.take() {
            self.failure(s.request, reason.into());
        }
        self.alias_checked = None;
    }
    fn finish_motion(&mut self, mut m: Motion, error: Option<String>) {
        if let Some(e) = error {
            m.command["unknown"] = true.into();
            m.command["status"] = "unknown".into();
            m.command["outcome"] = "unknown".into();
            m.command["error"] = e.clone().into();
            self.error = Some(e);
            if let Some(o) = self.owned.get_mut(&m.id) {
                o.cleanup = true;
                o.retry = Instant::now();
            }
        } else {
            m.command["status"] = "arrivalVerified".into();
            m.command["outcome"] = "arrivalVerified".into();
            m.command["arrivalVerified"] = true.into();
        }
        m.command["completedAt"] = epoch_ms().into();
        self.history.push_back(m.command.clone());
        while self.history.len() > 64 {
            self.history.pop_front();
        }
        self.response(m.request, json!({"command":m.command,"state":self.state()}));
        self.event();
    }
    fn cancel_if(&mut self, id: Option<u8>, actor: Option<&str>, reason: &str) {
        if self
            .motion
            .as_ref()
            .is_some_and(|m| id.is_none_or(|id| m.id == id) && actor.is_none_or(|a| m.actor == a))
        {
            let m = self.motion.take().unwrap();
            self.finish_motion(m, Some(reason.into()));
        }
    }
    fn mark_cleanup(&mut self, id: Option<u8>, actor: Option<&str>) {
        for (&i, o) in &mut self.owned {
            if id.is_none_or(|id| id == i) && actor.is_none_or(|a| a == o.actor) {
                o.cleanup = true;
                o.retry = Instant::now();
            }
        }
    }
    fn cleanup_id(&mut self, id: u8) {
        let Some(o) = self.owned.get(&id) else { return };
        let s = o.original.clone();
        let result = (|| -> Result<()> {
            // Torque-off first; never restore the old goal or old torque state.
            self.bus()?.write_verified(id, 64, &[0])?;
            // A latched watchdog blocks goal writes; clear it only after verified off.
            self.bus()?.write_verified(id, 98, &[0])?;
            self.bus()?.write_verified(id, 100, &s.r[36..38])?;
            self.bus()?.write_verified(id, 108, &s.r[44..48])?;
            self.bus()?.write_verified(id, 112, &s.r[48..52])?;
            self.bus()?.write_verified(id, 98, &s.r[34..35])?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.owned.remove(&id);
                if let Err(e) = self.refresh(id)
                    && let Some(s) = self.rows.get_mut(&id)
                {
                    s.online = false;
                    s.error = Some(e.to_string());
                }
            }
            Err(e) => {
                let msg = format!(
                    "ID {id} cleanup unconfirmed: {e:#}; remove external power if necessary"
                );
                self.error = Some(msg.clone());
                if let Some(s) = self.rows.get_mut(&id) {
                    s.online = false;
                    s.error = Some(msg);
                }
                if let Some(o) = self.owned.get_mut(&id) {
                    o.cleanup = true;
                    o.retry = Instant::now() + Duration::from_secs(1);
                }
            }
        }
    }
    fn cleanup_one(&mut self) {
        if let Some(id) = self
            .owned
            .iter()
            .find(|(_, o)| o.cleanup && o.retry <= Instant::now())
            .map(|(&id, _)| id)
        {
            self.cleanup_id(id);
        }
    }
    fn hazard(&mut self, reason: String) {
        self.error = Some(reason.clone());
        self.alias_checked = None;
        self.cancel_if(None, None, &reason);
        self.mark_cleanup(None, None);
    }
    fn step_scan(&mut self) {
        let Some(mut scan) = self.scan.take() else {
            return;
        };
        if Instant::now() < scan.after {
            self.scan = Some(scan);
            return;
        }
        let id = scan.next as u8;
        let result = (|| -> Result<Option<Snapshot>> {
            if !self.bus()?.ping(id)? {
                return Ok(None);
            }
            Ok(Some(self.snapshot(id)?))
        })();
        match result {
            Ok(Some(s)) => {
                if s.model() != 1200 || s.e[12] != 255 {
                    scan.valid = false;
                    self.error = Some(format!(
                        "ID {id}: unsupported model or SecondaryID; motion blocked"
                    ));
                }
                self.rows.insert(id, s);
            }
            Ok(None) => {
                if let Some(s) = self.rows.get_mut(&id) {
                    s.online = false;
                    s.error = Some("no ping response".into());
                    scan.valid = false;
                }
            }
            Err(e) => {
                scan.valid = false;
                self.error = Some(format!("scan ID {id}: {e:#}"));
                if let Some(s) = self.rows.get_mut(&id) {
                    s.online = false;
                    s.error = self.error.clone();
                }
            }
        }
        scan.next += 1;
        if scan.next > scan.max as u16 {
            if scan.max == 252 {
                self.full_scan_at = Some(epoch_ms());
                if scan.valid && !self.rows.is_empty() {
                    self.alias_checked = Some(Instant::now());
                }
            }
            self.response(scan.request, self.state());
            self.event();
            self.next_poll = Instant::now();
        } else {
            self.scan = Some(scan);
        }
    }
    fn step_motion(&mut self) {
        let Some(mut m) = self.motion.take() else {
            return;
        };
        let result = (|| -> Result<bool> {
            ensure!(
                m.started.elapsed() < Duration::from_secs(6),
                "arrival not verified within 6 seconds"
            );
            ensure!(
                self.owned.get(&m.id).is_some_and(|o| !o.cleanup),
                "ownership lost/cleanup pending"
            );
            let saved = m.baseline.clone();
            match m.phase {
                0 => {} // Preserve pre-existing near-stationary holding torque; do not unload the joint.
                1 => self
                    .bus()?
                    .write_verified(m.id, 100, &saved.pwm().min(200).to_le_bytes())?,
                2 => {
                    let a = if saved.acceleration() == 0 {
                        10
                    } else {
                        saved.acceleration().min(10)
                    };
                    self.bus()?.write_verified(m.id, 108, &a.to_le_bytes())?;
                }
                3 => {
                    let v = saved.velocity_cap();
                    self.bus()?.write_verified(m.id, 112, &v.to_le_bytes())?;
                }
                4 => self.bus()?.write_verified(m.id, 98, &[50])?,
                5 => {
                    self.refresh(m.id)?;
                    let s = &self.rows[&m.id];
                    ensure!(s.r[0] == saved.r[0], "unexpected torque state change");
                    s.holding_takeover()?;
                    let target = s.target(&m.actor, m.requested)?;
                    ensure!(
                        target == m.target,
                        "fresh position changed actor bounds; request a new move"
                    );
                    let p = s.position();
                    self.bus()?.write_verified(m.id, 116, &p.to_le_bytes())?;
                }
                6 => {
                    if saved.r[0] == 0 {
                        self.bus()?.write_verified(m.id, 64, &[1])?;
                    }
                }
                7 => {
                    self.refresh(m.id)?;
                    let s = &self.rows[&m.id];
                    let target = s.target(&m.actor, m.requested)?;
                    ensure!(
                        s.r[0] == 1 && target == m.target,
                        "post-enable safety/bounds changed"
                    );
                    self.bus()?
                        .write_verified(m.id, 116, &m.target.to_le_bytes())?;
                    m.sent = Some(Instant::now());
                    m.command["readbackVerified"] = true.into();
                    m.command["status"] = "readbackVerified".into();
                }
                _ => {
                    let s = &self.rows[&m.id];
                    if m.sent.is_none_or(|sent| s.at <= sent) {
                        return Ok(false);
                    }
                    s.safety()?;
                    ensure!(
                        s.r[0] == 1 && s.goal() == m.target,
                        "torque/goal changed during motion"
                    );
                    if s.at >= m.started
                        && (s.position() as f64 - m.target as f64).abs() * DEG <= 1.0
                    {
                        return Ok(true);
                    }
                }
            }
            if m.phase < 8 {
                m.phase += 1;
            }
            Ok(false)
        })();
        match result {
            Ok(true) => self.finish_motion(m, None),
            Ok(false) => self.motion = Some(m),
            Err(e) => self.finish_motion(m, Some(format!("{e:#}"))),
        }
    }
    pub fn tick(&mut self) {
        let expired: Vec<_> = self
            .owned
            .iter()
            .filter(|(_, o)| !o.cleanup && o.lease.elapsed() >= Duration::from_secs(5))
            .map(|(&id, _)| id)
            .collect();
        for id in expired {
            self.cancel_if(Some(id), None, "actor heartbeat lease expired");
            self.mark_cleanup(Some(id), None);
        }
        self.cleanup_one();
        if self.bus.is_none() {
            return;
        }
        if self.scan.is_some() {
            self.step_scan();
        } else {
            if self.poll_queue.is_empty() && Instant::now() >= self.next_poll {
                self.poll_queue = self
                    .rows
                    .keys()
                    .filter(|id| !self.owned.contains_key(id))
                    .copied()
                    .collect();
                self.next_poll = Instant::now() + Duration::from_secs_f64(1.0 / 3.0);
            }
            let priority = self
                .owned
                .iter()
                .filter(|(_, o)| !o.cleanup)
                .filter_map(|(&id, _)| self.rows.get(&id).map(|s| (id, s.at)))
                .filter(|(_, at)| at.elapsed() >= Duration::from_secs_f64(1.0 / 3.0))
                .min_by_key(|(_, at)| *at)
                .map(|(id, _)| id);
            let poll_id = priority.or_else(|| self.poll_queue.pop_front());
            if let Some(id) = poll_id {
                match self.refresh(id) {
                    Ok(()) => {
                        let s = &self.rows[&id];
                        let check = (|| -> Result<()> {
                            if s.model() == 1200 {
                                s.telemetry_safety()?;
                            }
                            if let Some(o) = self.owned.get(&id).filter(|o| !o.cleanup) {
                                s.safety()?;
                                if o.actor == "agent" {
                                    ensure!(
                                        (170.0..=190.0).contains(&(s.position() as f64 * DEG)),
                                        "agent present left170..190 degrees"
                                    );
                                }
                            }
                            Ok(())
                        })();
                        if let Err(e) = check {
                            let reason = format!("ID {id} safety: {e:#}");
                            if let Some(s) = self.rows.get_mut(&id) {
                                s.error = Some(reason.clone());
                            }
                            self.hazard(reason);
                        }
                    }
                    Err(e) => {
                        let reason = format!("ID {id} poll failed: {e:#}");
                        if let Some(s) = self.rows.get_mut(&id) {
                            s.online = false;
                            s.error = Some(reason.clone());
                        }
                        self.hazard(reason);
                    }
                }
            }
            let stale = self.owned.iter().any(|(id, o)| {
                !o.cleanup
                    && self
                        .rows
                        .get(id)
                        .is_none_or(|s| s.at.elapsed() > Duration::from_millis(750))
            });
            if stale {
                self.hazard("controlled feedback older than 750ms".into());
            }
            self.step_motion();
        }
        if Instant::now() >= self.next_event {
            self.event();
            self.next_event = Instant::now() + Duration::from_secs_f64(1.0 / 3.0);
        }
    }
    pub fn shutdown(&mut self) -> bool {
        self.cancel_scan("worker shutdown");
        self.cancel_if(None, None, "worker shutdown");
        self.mark_cleanup(None, None);
        for _ in 0..3 {
            let ids: Vec<_> = self.owned.keys().copied().collect();
            for id in ids {
                self.cleanup_id(id);
            }
            if self.owned.is_empty() {
                break;
            }
        }
        self.event();
        self.owned.is_empty()
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        if !self.owned.is_empty() {
            let ids: Vec<_> = self.owned.keys().copied().collect();
            for id in ids {
                self.cleanup_id(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    include!("engine_tests.rs");
}
