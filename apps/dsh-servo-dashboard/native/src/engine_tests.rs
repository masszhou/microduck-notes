use super::*;
use crate::tests::memory;
use std::{cell::RefCell, rc::Rc};
#[derive(Default)]
struct Fake {
    mem: BTreeMap<u8, Vec<u8>>,
    writes: Vec<(u8, u8, Vec<u8>)>,
    fail_write: Option<u8>,
    fail_read: Option<u8>,
    follows: bool,
}
struct Mock(Rc<RefCell<Fake>>);
impl Bus for Mock {
    fn ping(&mut self, id: u8) -> Result<bool> {
        Ok(self.0.borrow().mem.contains_key(&id))
    }
    fn read(&mut self, id: u8, a: u8, n: u8) -> Result<Vec<u8>> {
        let f = self.0.borrow();
        ensure!(f.fail_read != Some(id), "injected read fault");
        let b = f.mem.get(&id).ok_or_else(|| anyhow!("missing ID"))?;
        Ok(b[a as usize..a as usize + n as usize].to_vec())
    }
    fn write_verified(&mut self, id: u8, a: u8, data: &[u8]) -> Result<()> {
        let mut f = self.0.borrow_mut();
        f.writes.push((id, a, data.to_vec()));
        ensure!(f.fail_write != Some(a), "injected write uncertainty");
        let follows = f.follows;
        let b = f.mem.get_mut(&id).ok_or_else(|| anyhow!("missing ID"))?;
        b[a as usize..a as usize + data.len()].copy_from_slice(data);
        if a == 116 && follows {
            b[132..136].copy_from_slice(data);
        }
        Ok(())
    }
}
fn setup() -> (Engine, Rc<RefCell<Fake>>) {
    let f = Rc::new(RefCell::new(Fake {
        follows: true,
        ..Fake::default()
    }));
    for id in [1, 2, 3, 11, 12] {
        let mut b = memory(id);
        if id >= 11 {
            b[64] = 1;
        }
        f.borrow_mut().mem.insert(id, b);
    }
    let mut e = Engine::default();
    e.bus = Some(Box::new(Mock(f.clone())));
    for id in [1, 2, 3, 11, 12] {
        let s = e.snapshot(id).unwrap();
        e.rows.insert(id, s);
    }
    e.alias_checked = Some(Instant::now());
    (e, f)
}
fn moving(e: &mut Engine, id: u8, actor: &str) {
    e.handle(
        json!(format!("move-{id}")),
        Action::Move {
            id,
            deg: 181.0,
            actor: actor.into(),
        },
    );
    assert!(e.motion.is_some(), "{:?}", e.outputs);
}
fn finish(e: &mut Engine) {
    for _ in 0..20 {
        e.step_motion();
        if let Some(m) = &e.motion
            && m.phase == 8
        {
            let id = m.id;
            e.refresh(id).unwrap();
        }
    }
    assert!(e.motion.is_none());
}
#[test]
fn discovery_and_poll_never_write() {
    let (mut e, f) = setup();
    e.handle(json!(1), Action::Scan { max: 252 });
    while e.scan.is_some() {
        e.step_scan();
    }
    for _ in 0..30 {
        e.tick();
    }
    assert!(f.borrow().writes.is_empty());
    assert!(e.alias_checked.is_some());
    assert_eq!(f.borrow().mem[&11][64], 1);
}
#[test]
fn move_readback_arrival_then_release_restores_only_touched() {
    let (mut e, f) = setup();
    moving(&mut e, 1, "agent");
    finish(&mut e);
    assert_eq!(e.history.back().unwrap()["arrivalVerified"], true);
    assert_eq!(f.borrow().mem[&1][64], 1);
    assert_eq!(f.borrow().mem[&1][98], 50);
    assert_eq!(
        u32::from_le_bytes(f.borrow().mem[&1][112..116].try_into().unwrap()),
        5
    );
    e.handle(
        json!(2),
        Action::Release {
            id: 1,
            actor: "agent".into(),
        },
    );
    assert!(e.owned.is_empty());
    let f = f.borrow();
    assert!(f.writes.iter().all(|w| w.0 == 1));
    assert_eq!(f.mem[&1][64], 0);
    assert_eq!(f.mem[&1][100..116], memory(1)[100..116]);
    assert_eq!(f.mem[&11][64], 1);
    assert_eq!(f.mem[&12][64], 1);
}
#[test]
fn uncertain_goal_never_auto_resends() {
    let (mut e, f) = setup();
    f.borrow_mut().fail_write = Some(116);
    moving(&mut e, 1, "agent");
    finish(&mut e);
    assert_eq!(e.history.back().unwrap()["unknown"], true);
    e.cleanup_one();
    for _ in 0..10 {
        e.step_motion();
    }
    assert_eq!(f.borrow().writes.iter().filter(|w| w.1 == 116).count(), 1);
}
#[test]
fn dirty_survives_failed_cleanup_and_retries() {
    let (mut e, f) = setup();
    moving(&mut e, 1, "agent");
    finish(&mut e);
    f.borrow_mut().fail_write = Some(100);
    e.handle(
        json!(2),
        Action::Release {
            id: 1,
            actor: "agent".into(),
        },
    );
    assert!(e.owned[&1].cleanup);
    assert_eq!(f.borrow().mem[&1][64], 0);
    f.borrow_mut().fail_write = None;
    e.cleanup_id(1);
    assert!(e.owned.is_empty());
}
#[test]
fn missing_ids_alias_and_wrong_owner_zero_writes() {
    let (mut e, f) = setup();
    e.handle(
        json!(1),
        Action::Move {
            id: 9,
            deg: 180.0,
            actor: "human".into(),
        },
    );
    e.alias_checked = None;
    e.handle(
        json!(2),
        Action::Move {
            id: 1,
            deg: 180.0,
            actor: "agent".into(),
        },
    );
    assert!(f.borrow().writes.is_empty());
    e.alias_checked = Some(Instant::now());
    moving(&mut e, 1, "human");
    finish(&mut e);
    f.borrow_mut().writes.clear();
    e.handle(
        json!(3),
        Action::Move {
            id: 1,
            deg: 180.0,
            actor: "agent".into(),
        },
    );
    e.handle(
        json!(4),
        Action::Release {
            id: 1,
            actor: "agent".into(),
        },
    );
    e.handle(
        json!(5),
        Action::Stop {
            actor: "agent".into(),
        },
    );
    assert!(f.borrow().writes.is_empty());
    assert_eq!(e.owned[&1].actor, "human");
}
#[test]
fn lease_is_actor_specific_and_poll_does_not_renew() {
    let (mut e, _f) = setup();
    moving(&mut e, 1, "agent");
    finish(&mut e);
    let before = Instant::now() - Duration::from_secs(4);
    e.owned.get_mut(&1).unwrap().lease = before;
    e.handle(
        json!(2),
        Action::Heartbeat {
            actor: "human".into(),
        },
    );
    e.refresh(1).unwrap();
    assert_eq!(e.owned[&1].lease, before);
    e.owned.get_mut(&1).unwrap().lease = Instant::now() - Duration::from_secs(6);
    e.handle(
        json!(3),
        Action::Heartbeat {
            actor: "agent".into(),
        },
    );
    e.tick();
    assert!(e.owned.is_empty());
}
#[test]
fn human_stop_all_touched_agent_stop_only_agent() {
    let (mut e, f) = setup();
    moving(&mut e, 1, "agent");
    finish(&mut e);
    moving(&mut e, 2, "human");
    finish(&mut e);
    e.handle(
        json!(2),
        Action::Stop {
            actor: "agent".into(),
        },
    );
    assert!(!e.owned.contains_key(&1));
    assert!(e.owned.contains_key(&2));
    e.handle(
        json!(3),
        Action::Stop {
            actor: "human".into(),
        },
    );
    while e.owned.values().any(|o| o.cleanup) {
        e.cleanup_one();
    }
    assert!(e.owned.is_empty());
    assert!(f.borrow().writes.iter().all(|w| w.0 <= 2));
}
#[test]
fn timeout_poll_fault_and_shutdown_are_touched_only() {
    let (mut e, f) = setup();
    moving(&mut e, 1, "agent");
    e.motion.as_mut().unwrap().started = Instant::now() - Duration::from_secs(7);
    e.step_motion();
    assert_eq!(e.history.back().unwrap()["unknown"], true);
    e.cleanup_one();
    moving(&mut e, 2, "human");
    finish(&mut e);
    f.borrow_mut().fail_read = Some(11);
    e.poll_queue = VecDeque::from([11]);
    e.tick();
    assert!(e.owned[&2].cleanup);
    f.borrow_mut().fail_read = None;
    assert!(e.shutdown());
    assert!(f.borrow().writes.iter().all(|w| w.0 <= 2));
}
#[test]
fn scan_refuses_while_owned_and_aliases_block() {
    let (mut e, f) = setup();
    moving(&mut e, 1, "agent");
    finish(&mut e);
    f.borrow_mut().writes.clear();
    e.handle(json!(2), Action::Scan { max: 252 });
    assert!(e.scan.is_none());
    assert!(f.borrow().writes.is_empty());
    e.handle(
        json!(3),
        Action::Stop {
            actor: "human".into(),
        },
    );
    f.borrow_mut().mem.get_mut(&12).unwrap()[12] = 1;
    e.handle(json!(4), Action::Scan { max: 252 });
    while e.scan.is_some() {
        e.step_scan();
    }
    assert!(e.alias_checked.is_none());
    f.borrow_mut().writes.clear();
    e.handle(
        json!(5),
        Action::Move {
            id: 1,
            deg: 180.0,
            actor: "agent".into(),
        },
    );
    assert!(f.borrow().writes.is_empty());
}
#[test]
fn initially_torqued_explicit_control_ends_off() {
    let (mut e, f) = setup();
    moving(&mut e, 11, "human");
    finish(&mut e);
    assert!(f.borrow().writes.iter().all(|w| w.1 != 64));
    assert_eq!(f.borrow().mem[&11][64], 1);
    assert!(e.shutdown());
    assert_eq!(f.borrow().mem[&11][64], 0);
    assert_eq!(f.borrow().mem[&12][64], 1);
}
#[test]
fn moving_or_distant_torque_on_takeover_refuses_without_writes() {
    for (a, data) in [
        (128, 2i32.to_le_bytes()),
        (128, (-2i32).to_le_bytes()),
        (128, i32::MIN.to_le_bytes()),
        (128, i32::MAX.to_le_bytes()),
        (116, 2500i32.to_le_bytes()),
    ] {
        let (mut e, f) = setup();
        f.borrow_mut().mem.get_mut(&11).unwrap()[a..a + 4].copy_from_slice(&data);
        e.handle(
            json!(1),
            Action::Move {
                id: 11,
                deg: 181.0,
                actor: "human".into(),
            },
        );
        assert!(e.motion.is_none());
        assert!(f.borrow().writes.is_empty());
        assert_eq!(f.borrow().mem[&11][64], 1);
    }
}
#[test]
fn raw_one_tick_velocity_near_stationary_takeover_preserves_torque() {
    for raw in [-1i32, 1] {
        let (mut e, f) = setup();
        f.borrow_mut().mem.get_mut(&11).unwrap()[128..132].copy_from_slice(&raw.to_le_bytes());
        moving(&mut e, 11, "human");
        finish(&mut e);
        assert_eq!(e.history.back().unwrap()["arrivalVerified"], true);
        assert!(f.borrow().writes.iter().all(|w| w.1 != 64));
        assert_eq!(f.borrow().mem[&11][64], 1);
        assert!(e.shutdown());
        assert_eq!(f.borrow().mem[&11][64], 0);
        assert_eq!(f.borrow().mem[&12][64], 1);
    }
}
#[test]
fn consecutive_agent_moves_with_raw_one_keep_lease_and_original_ram() {
    let (mut e, f) = setup();
    f.borrow_mut().mem.get_mut(&1).unwrap()[128..132].copy_from_slice(&1i32.to_le_bytes());
    moving(&mut e, 1, "agent");
    finish(&mut e);
    assert_eq!(e.history.back().unwrap()["arrivalVerified"], true);
    e.handle(
        json!("heartbeat"),
        Action::Heartbeat {
            actor: "agent".into(),
        },
    );
    let lease = e.owned[&1].lease;
    let first_write_count = f.borrow().writes.len();
    e.handle(
        json!("second-move"),
        Action::Move {
            id: 1,
            deg: 180.0,
            actor: "agent".into(),
        },
    );
    assert!(e.motion.is_some());
    finish(&mut e);
    assert_eq!(e.history.back().unwrap()["arrivalVerified"], true);
    assert_eq!(e.owned[&1].lease, lease);
    assert!(
        f.borrow().writes[first_write_count..]
            .iter()
            .all(|w| w.1 != 64)
    );
    assert_eq!(f.borrow().mem[&1][64], 1);
    e.handle(
        json!("release"),
        Action::Release {
            id: 1,
            actor: "agent".into(),
        },
    );
    assert!(e.owned.is_empty());
    let f = f.borrow();
    assert_eq!(f.mem[&1][64], 0);
    assert_eq!(f.mem[&1][100..116], memory(1)[100..116]);
    assert!(f.writes.iter().all(|w| w.0 == 1));
    assert_eq!(f.mem[&11][64], 1);
    assert_eq!(f.mem[&12][64], 1);
}
#[test]
fn conservative_profile_caps_existing_and_eeprom() {
    let (mut e, f) = setup();
    {
        let mut f = f.borrow_mut();
        let b = f.mem.get_mut(&1).unwrap();
        b[44..48].copy_from_slice(&2u32.to_le_bytes());
        b[108..112].copy_from_slice(&0u32.to_le_bytes());
        b[112..116].copy_from_slice(&0u32.to_le_bytes());
        b[100..102].copy_from_slice(&100u16.to_le_bytes());
    }
    moving(&mut e, 1, "agent");
    finish(&mut e);
    {
        let f = f.borrow();
        let b = &f.mem[&1];
        assert_eq!(b[100..102], 100u16.to_le_bytes());
        assert_eq!(b[108..112], 10u32.to_le_bytes());
        assert_eq!(b[112..116], 2u32.to_le_bytes());
    }
    assert!(e.shutdown());
}
#[test]
fn untouched_hardware_error_releases_only_touched() {
    let (mut e, f) = setup();
    moving(&mut e, 1, "agent");
    finish(&mut e);
    f.borrow_mut().mem.get_mut(&11).unwrap()[70] = 1;
    e.poll_queue = VecDeque::from([11]);
    e.tick();
    assert!(e.owned[&1].cleanup);
    e.cleanup_one();
    assert!(e.owned.is_empty());
    assert!(f.borrow().writes.iter().all(|w| w.0 == 1));
    assert_eq!(f.borrow().mem[&11][64], 1);
}
#[test]
fn agent_outside_bounds_triggers_release() {
    let (mut e, f) = setup();
    moving(&mut e, 1, "agent");
    finish(&mut e);
    f.borrow_mut().mem.get_mut(&1).unwrap()[132..136].copy_from_slice(&2200i32.to_le_bytes());
    e.poll_queue = VecDeque::from([1]);
    e.tick();
    assert!(e.owned[&1].cleanup);
    e.cleanup_one();
    assert!(e.owned.is_empty());
}
#[test]
fn explicit_human_release_unowned_torqued_motor_touches_only_selected_id() {
    let (mut e, f) = setup();
    f.borrow_mut().mem.get_mut(&11).unwrap()[116..120].copy_from_slice(&1900i32.to_le_bytes());
    e.handle(
        json!("release-agent"),
        Action::Release {
            id: 11,
            actor: "agent".into(),
        },
    );
    assert!(f.borrow().writes.is_empty());
    assert!(e.owned.is_empty());
    e.handle(
        json!("release-human"),
        Action::Release {
            id: 11,
            actor: "human".into(),
        },
    );
    assert!(e.owned.is_empty());
    let f = f.borrow();
    assert!(!f.writes.is_empty());
    assert!(f.writes.iter().all(|w| w.0 == 11 && w.1 != 116));
    assert_eq!(f.mem[&11][64], 0);
    assert_eq!(f.mem[&12][64], 1);
    assert_eq!(f.mem[&11][116..120], 1900i32.to_le_bytes());
    assert_eq!(f.mem[&11][100..116], memory(11)[100..116]);
}
#[test]
fn unowned_human_release_requires_alias_certificate_and_compatible_snapshot() {
    for condition in [
        "no-scan",
        "expired-scan",
        "alias",
        "bad-mode",
        "undiscovered",
    ] {
        let (mut e, f) = setup();
        let mut id = 11;
        match condition {
            "no-scan" => e.alias_checked = None,
            "expired-scan" => e.alias_checked = Some(Instant::now() - Duration::from_secs(61)),
            "alias" => e.rows.get_mut(&12).unwrap().e[12] = 11,
            "bad-mode" => f.borrow_mut().mem.get_mut(&11).unwrap()[11] = 4,
            "undiscovered" => id = 99,
            _ => unreachable!(),
        }
        e.handle(
            json!(condition),
            Action::Release {
                id,
                actor: "human".into(),
            },
        );
        assert!(f.borrow().writes.is_empty(), "{condition}");
        assert!(e.owned.is_empty());
        assert_eq!(f.borrow().mem[&11][64], 1);
    }
}
#[test]
fn unowned_release_failed_restore_retains_originals_for_retry() {
    let (mut e, f) = setup();
    f.borrow_mut().fail_write = Some(100);
    e.handle(
        json!("release"),
        Action::Release {
            id: 11,
            actor: "human".into(),
        },
    );
    assert_eq!(e.owned[&11].actor, "human");
    assert!(e.owned[&11].cleanup);
    assert_eq!(f.borrow().mem[&11][64], 0);
    f.borrow_mut().fail_write = None;
    e.cleanup_id(11);
    assert!(e.owned.is_empty());
    assert_eq!(f.borrow().mem[&11][100..116], memory(11)[100..116]);
    assert_eq!(f.borrow().mem[&12][64], 1);
    assert!(f.borrow().writes.iter().all(|w| w.0 == 11));
}
#[test]
fn full_scan_expires_without_writes() {
    let (mut e, f) = setup();
    e.alias_checked = Some(Instant::now() - Duration::from_secs(61));
    e.handle(
        json!(1),
        Action::Move {
            id: 1,
            deg: 181.0,
            actor: "agent".into(),
        },
    );
    assert!(f.borrow().writes.is_empty());
    assert_eq!(e.state()["fullScanValid"], false);
}
