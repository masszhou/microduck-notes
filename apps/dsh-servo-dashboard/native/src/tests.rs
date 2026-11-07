use crate::model::{DEG, Snapshot, parse};
use serde_json::json;

pub fn memory(id: u8) -> Vec<u8> {
    let mut b = vec![0; 147];
    b[0..2].copy_from_slice(&1200u16.to_le_bytes());
    b[6] = 42;
    b[7] = id;
    b[11] = 3;
    b[12] = 255;
    b[44..48].copy_from_slice(&100u32.to_le_bytes());
    b[48..52].copy_from_slice(&4095u32.to_le_bytes());
    b[68] = 2;
    b[100..102].copy_from_slice(&400u16.to_le_bytes());
    b[108..112].copy_from_slice(&20u32.to_le_bytes());
    b[112..116].copy_from_slice(&30u32.to_le_bytes());
    b[116..120].copy_from_slice(&2048i32.to_le_bytes());
    b[132..136].copy_from_slice(&2048i32.to_le_bytes());
    b[144..146].copy_from_slice(&50u16.to_le_bytes());
    b[146] = 25;
    b
}
fn snapshot(b: Vec<u8>) -> Snapshot {
    Snapshot::new(b[7], b[..64].to_vec(), b[64..].to_vec()).unwrap()
}
#[test]
fn validates_json_types_before_io() {
    for v in [
        json!({}),
        json!({"requestId":1,"action":"move","id":1.5,"positionDeg":180,"actor":"agent"}),
        json!({"requestId":1,"action":"move","id":1,"positionDeg":"180","actor":"agent"}),
        json!({"requestId":1,"action":"move","id":4,"positionDeg":180,"actor":"agent"}),
        json!({"requestId":1,"action":"move","id":1,"positionDeg":169.99,"actor":"agent"}),
        json!({"requestId":1,"action":"move","id":1,"positionDeg":180,"actor":"bot"}),
        json!({"requestId":1,"action":"scan","maxId":null}),
        json!({"requestId":1,"action":"status","torque":1}),
        json!({"requestId":1,"action":"connect","port":"x","baud":1000000.1}),
    ] {
        assert!(parse(&v).is_err(), "{v}");
    }
    assert!(
        parse(&json!({"requestId":"r","action":"move","id":1,"positionDeg":180,"actor":"agent"}))
            .is_ok()
    );
}
#[test]
fn inward_quantization_and_human_limit() {
    let s = snapshot(memory(1));
    assert!(s.target("agent", 170.0).unwrap() as f64 * DEG >= 170.0);
    assert!(s.target("agent", 190.0).unwrap() as f64 * DEG <= 190.0);
    assert!(s.target("human", 190.0001).is_err());
    for present in 1i32..4095 {
        let mut b = memory(5);
        b[132..136].copy_from_slice(&present.to_le_bytes());
        let s = snapshot(b);
        for d in [-10.0, 10.0] {
            let target = present as f64 * DEG + d;
            if (0.0..=4095.0 * DEG).contains(&target) {
                let t = s.target("human", target).unwrap();
                assert!((t - present).abs() as f64 * DEG <= 10.0);
            }
        }
    }
    assert!(s.target("agent", f64::NAN).is_err());
    assert!(s.target("human", f64::INFINITY).is_err());
    let mut b = memory(1);
    b[132..136].copy_from_slice(&2071i32.to_le_bytes());
    let s = snapshot(b);
    assert!(s.target("agent", 170.0).is_err());
    assert!(s.target("agent", 190.0).is_ok());
}
#[test]
fn all_safety_failures() {
    for (addr, data) in [
        (0, vec![0, 0]),
        (11, vec![4]),
        (10, vec![4]),
        (10, vec![8]),
        (12, vec![1]),
        (20, vec![1, 0, 0, 0]),
        (48, 5000u32.to_le_bytes().to_vec()),
        (52, 4095u32.to_le_bytes().to_vec()),
        (132, (-1i32).to_le_bytes().to_vec()),
        (68, vec![0]),
        (70, vec![1]),
        (98, vec![255]),
        (144, vec![41, 0]),
        (144, vec![59, 0]),
        (146, vec![55]),
        (126, 350i16.to_le_bytes().to_vec()),
        (126, (-350i16).to_le_bytes().to_vec()),
        (100, vec![0, 0]),
    ] {
        let mut b = memory(1);
        b[addr..addr + data.len()].copy_from_slice(&data);
        assert!(snapshot(b).safety().is_err(), "address {addr}");
    }
    let mut b = memory(1);
    b[132..136].copy_from_slice(&1500i32.to_le_bytes());
    assert!(snapshot(b).target("agent", 180.0).is_err());
}
