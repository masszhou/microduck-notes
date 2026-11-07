use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::f64::consts::{PI, TAU};

pub const IDS: [u8; 15] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
// Logical slot order mirrors microduck, NOT a claim about physical joint calibration.
pub const POLICY_IDS: [u8; 14] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15];
pub const MOUTH: usize = 9;
pub const TICK_RAD: f64 = TAU / 4096.0;

#[derive(Clone, Copy, Debug, clap::Args)]
pub struct Profile {
    /// Profile Velocity (112), raw units: 1 = 0.229 rpm = 1.374 deg/s.
    #[arg(long, default_value_t = 30)]
    pub profile_velocity: u32,
    /// Profile Acceleration (108), raw units: 1 = 214.577 rpm/min = 21.4577 deg/s^2.
    #[arg(long, default_value_t = 20)]
    pub profile_acceleration: u32,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            profile_velocity: 30,
            profile_acceleration: 20,
        }
    }
}

impl Profile {
    pub fn validate(self) -> Result<()> {
        ensure!(
            (1..=32767).contains(&self.profile_velocity),
            "profile-velocity must be 1..32767 (raw register units)"
        );
        ensure!(
            (1..=32767).contains(&self.profile_acceleration),
            "profile-acceleration must be 1..32767 (raw register units)"
        );
        Ok(())
    }
    pub fn velocity_deg_s(self) -> f64 {
        self.profile_velocity as f64 * 1.374
    }
    pub fn acceleration_deg_s2(self) -> f64 {
        self.profile_acceleration as f64 * 21.4577
    }
    pub fn bytes(self) -> Vec<u8> {
        [
            self.profile_acceleration.to_le_bytes(),
            self.profile_velocity.to_le_bytes(),
        ]
        .concat()
    }
}

#[derive(Clone, Debug)]
pub struct Sample {
    pub ticks: [i32; 15],
    pub velocity: [f64; 15],
    pub current_ma: [f64; 15],
    pub pwm: [i16; 15],
    pub volts: [f64; 15],
    pub temperature: [u8; 15],
}

impl Sample {
    pub fn decode(blocks: &[Vec<u8>]) -> Result<Self> {
        ensure!(blocks.len() == 15, "expected 15 feedback blocks");
        let mut s = Self::simulated([2048; 15]);
        for (i, b) in blocks.iter().enumerate() {
            ensure!(
                b.len() == 23,
                "ID {}: expected 23 bytes, got {}",
                IDS[i],
                b.len()
            );
            s.pwm[i] = i16::from_le_bytes(b[0..2].try_into()?);
            s.current_ma[i] = i16::from_le_bytes(b[2..4].try_into()?) as f64;
            s.velocity[i] = i32::from_le_bytes(b[4..8].try_into()?) as f64 * 0.229 * TAU / 60.0;
            s.ticks[i] = i32::from_le_bytes(b[8..12].try_into()?);
            s.volts[i] = u16::from_le_bytes(b[20..22].try_into()?) as f64 * 0.1;
            s.temperature[i] = b[22];
        }
        Ok(s)
    }

    pub fn simulated(ticks: [i32; 15]) -> Self {
        Self {
            ticks,
            velocity: [0.0; 15],
            current_ma: [0.0; 15],
            pwm: [0; 15],
            volts: [5.0; 15],
            temperature: [25; 15],
        }
    }

    pub fn radians(&self) -> [f64; 15] {
        self.ticks.map(|p| p as f64 * TICK_RAD - PI)
    }

    pub fn check(&self) -> Result<()> {
        for (i, id) in IDS.iter().enumerate() {
            ensure!(
                (4.2..=5.8).contains(&self.volts[i]),
                "ID {} voltage {:.1} V outside bench range 4.2..5.8 V",
                id,
                self.volts[i]
            );
            ensure!(
                self.temperature[i] < 55,
                "ID {} temperature {} C >= 55 C",
                id,
                self.temperature[i]
            );
            ensure!(
                self.current_ma[i].abs() < 350.0,
                "ID {} current {:.0} mA >= 350 mA",
                id,
                self.current_ma[i]
            );
        }
        let total: f64 = self.current_ma.iter().map(|v| v.abs()).sum();
        ensure!(
            total < 2000.0,
            "sum of servo current readings {:.0} mA >= 2000 mA",
            total
        );
        Ok(())
    }

    pub fn json(&self) -> Value {
        json!({"ids": IDS, "position_ticks": self.ticks, "position_rad": self.radians(),
            "velocity_rad_s": self.velocity, "current_ma": self.current_ma, "pwm_raw": self.pwm,
            "voltage_v": self.volts, "temperature_c": self.temperature})
    }
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Pattern {
    Wave,
    Mirror,
}

impl Pattern {
    pub fn name(self) -> &'static str {
        match self {
            Self::Wave => "wave",
            Self::Mirror => "mirror",
        }
    }
}

pub struct Wave {
    pub start: [i32; 15],
    pub home: [i32; 15],
    pub scale: f64,
    pub period: f64,
    pub pattern: Pattern,
}

impl Wave {
    pub fn new(start: [i32; 15], amplitude_deg: f64, period: f64, pattern: Pattern) -> Self {
        let scale = amplitude_deg.to_radians();
        let margin = (scale / TICK_RAD).ceil() as i32 + 2;
        let home = start.map(|p| p.clamp(margin, 4095 - margin));
        Self {
            start,
            home,
            scale,
            period,
            pattern,
        }
    }

    pub fn target(&self, t: f64) -> [i32; 15] {
        // Smooth 4-second fade-in, all motors start exactly where they were read.
        let u = (t / 4.0).clamp(0.0, 1.0);
        let fade = u * u * (3.0 - 2.0 * u);
        std::array::from_fn(|i| {
            let (weight, phase) = match self.pattern {
                Pattern::Wave => (
                    (self.scale * [1.0, 0.75, 0.5][i % 3])
                        .max(30.0f64.to_radians())
                        .min(self.scale)
                        / self.scale,
                    TAU * i as f64 / 15.0,
                ),
                // All 15 use the full amplitude; odd/even IDs sweep in opposite directions.
                Pattern::Mirror => (1.0, if i % 2 == 0 { 0.0 } else { PI }),
            };
            let wave = self.scale / TICK_RAD * weight * (TAU * t / self.period + phase).sin();
            (self.start[i] as f64 + fade * (self.home[i] as f64 - self.start[i] as f64 + wave))
                .round() as i32
        })
    }

    pub fn action(&self, target: &[i32; 15]) -> [f32; 14] {
        std::array::from_fn(|slot| {
            let j = joint(slot);
            ((target[j] - self.home[j]) as f64 * TICK_RAD / self.scale) as f32
        })
    }

    pub fn observation(&self, s: &Sample, previous: &[f32; 14]) -> [f32; 61] {
        let mut obs = [0.0; 61];
        obs[5] = -1.0; // SYNTHETIC upright gravity. No IMU is attached.
        for slot in 0..14 {
            let j = joint(slot);
            obs[6 + slot] = ((s.ticks[j] - self.home[j]) as f64 * TICK_RAD) as f32;
            obs[20 + slot] = s.velocity[j] as f32;
            obs[34 + slot] = previous[slot];
        }
        // 48..61: zero command by design; no locomotion request in a bench test.
        obs
    }
}

fn joint(slot: usize) -> usize {
    if slot < MOUTH { slot } else { slot + 1 }
}

pub fn validate_motion(
    hz: f64,
    duration: f64,
    amplitude: f64,
    period: f64,
    baud: u32,
    profile: Profile,
) -> Result<()> {
    profile.validate()?;
    ensure!(
        hz.is_finite() && (1.0..=20.0).contains(&hz),
        "hz must be 1..20"
    );
    ensure!(
        duration.is_finite() && (1.0..=3600.0).contains(&duration),
        "duration must be 1..3600 seconds"
    );
    ensure!(
        amplitude.is_finite() && (30.0..=90.0).contains(&amplitude),
        "amplitude-deg must be 30..90"
    );
    ensure!(
        period.is_finite() && (4.0..=60.0).contains(&period),
        "period must be 4..60 seconds"
    );
    ensure!(
        TAU * amplitude / period <= profile.velocity_deg_s(),
        "pattern peak speed {:.1} deg/s exceeds profile {:.1} deg/s; increase --period or --profile-velocity",
        TAU * amplitude / period,
        profile.velocity_deg_s()
    );
    // ~628 bytes/tick, 10 UART bits/byte + byte stuffing and turnaround margin.
    ensure!(
        hz * (8000.0 / baud as f64 + 0.015) <= 0.8,
        "requested rate too high for baud {baud}; use --hz 5 at 57600"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_feedback_and_physical_units() {
        let mut b = vec![0; 23];
        b[0..2].copy_from_slice(&(-20i16).to_le_bytes());
        b[2..4].copy_from_slice(&(-17i16).to_le_bytes());
        b[4..8].copy_from_slice(&(-100i32).to_le_bytes());
        b[8..12].copy_from_slice(&(2048i32).to_le_bytes());
        b[20..22].copy_from_slice(&(50u16).to_le_bytes());
        b[22] = 27;
        let s = Sample::decode(&vec![b; 15]).unwrap();
        assert_eq!(s.current_ma[0], -17.0);
        assert!((s.velocity[0] + 2.398082).abs() < 0.00001);
        assert_eq!(s.radians()[0], 0.0);
        assert_eq!(s.volts[0], 5.0);
        assert!(Sample::decode(&vec![vec![0; 22]; 15]).is_err());
    }

    #[test]
    fn rl_layout_mouth_exclusion_and_action_round_trip() {
        let w = Wave::new([2048; 15], 30.0, 8.0, Pattern::Wave);
        let mut s = Sample::simulated(std::array::from_fn(|i| 2048 + i as i32 * 10));
        s.velocity = std::array::from_fn(|i| i as f64);
        let previous = std::array::from_fn(|i| 0.01 * i as f32);
        let obs = w.observation(&s, &previous);
        assert_eq!(&obs[0..6], &[0.0, 0.0, 0.0, 0.0, 0.0, -1.0]);
        assert_eq!(obs[29], 10.0); // slot 9 follows the skipped ID 10.
        assert_eq!(&obs[34..48], &previous);
        assert_eq!(&obs[48..], &[0.0; 13]);
        let target = w.target(5.5);
        for (slot, a) in w.action(&target).iter().enumerate() {
            assert!(
                ((*a as f64 * w.scale / TICK_RAD + 2048.0) - target[joint(slot)] as f64).abs()
                    < 0.001
            );
        }
    }

    #[test]
    fn wave_starts_without_a_step_and_never_wraps() {
        for (start, pattern) in [0, 1, 100, 2048, 4094, 4095]
            .into_iter()
            .flat_map(|start| [Pattern::Wave, Pattern::Mirror].map(|p| (start, p)))
        {
            let w = Wave::new([start; 15], 45.0, 12.0, pattern);
            assert_eq!(w.target(0.0), [start; 15]);
            for step in 0..1000 {
                assert!(
                    w.target(step as f64 / 10.0)
                        .iter()
                        .all(|p| (0..=4095).contains(p))
                );
            }
        }
    }

    #[test]
    fn mirror_moves_every_motor_through_the_full_ninety_degree_span() {
        let w = Wave::new([2048; 15], 45.0, 12.0, Pattern::Mirror);
        let low = w.target(9.0);
        let high = w.target(15.0);
        for i in 0..15 {
            assert_eq!((high[i] - low[i]).abs(), 1024);
            assert_eq!(high[i] + low[i], 4096);
            assert_eq!(low[i], if i % 2 == 0 { 1536 } else { 2560 });
        }
        assert!(validate_motion(5.0, 30.0, 45.0, 12.0, 57600, Profile::default()).is_ok());
    }

    #[test]
    fn wave_minimum_amplitude_and_larger_single_turn_ranges() {
        for amplitude in [30.0, 45.0, 90.0] {
            let w = Wave::new([2048; 15], amplitude, 16.0, Pattern::Wave);
            for i in 0..15 {
                let phase_time = 16.0 * i as f64 / 15.0;
                let high = w.target(36.0 - phase_time)[i];
                let low = w.target(44.0 - phase_time)[i];
                assert!((high - low) as f64 * TICK_RAD >= 59.8f64.to_radians());
            }
        }
        for pattern in [Pattern::Wave, Pattern::Mirror] {
            for start in [0, 4095] {
                let w = Wave::new([start; 15], 90.0, 16.0, pattern);
                assert_eq!(w.target(0.0), [start; 15]);
                for step in 0..1000 {
                    assert!(
                        w.target(step as f64 / 10.0)
                            .iter()
                            .all(|p| (0..=4095).contains(p))
                    );
                }
            }
        }
    }

    #[test]
    fn rejects_faults_and_unachievable_rates() {
        let mut s = Sample::simulated([2048; 15]);
        s.volts[14] = 4.1;
        assert!(s.check().is_err());
        s.volts[14] = 5.0;
        s.current_ma = [140.0; 15];
        assert!(s.check().is_err());
        s.current_ma = [0.0; 15];
        s.current_ma[4] = -400.0;
        assert!(s.check().is_err());
        s.current_ma[4] = 0.0;
        s.temperature[7] = 55;
        assert!(s.check().is_err());
        assert!(validate_motion(5.0, 30.0, 30.0, 8.0, 57600, Profile::default()).is_ok());
        assert!(validate_motion(50.0, 30.0, 30.0, 8.0, 57600, Profile::default()).is_err());
        assert!(validate_motion(5.0, 30.0, f64::NAN, 8.0, 57600, Profile::default()).is_err());
        assert!(validate_motion(5.0, 30.0, 10.0, 8.0, 57600, Profile::default()).is_err());
        assert!(validate_motion(5.0, 30.0, 45.0, 8.0, 57600, Profile::default()).is_ok());
        assert!(validate_motion(5.0, 30.0, 90.0, 8.0, 57600, Profile::default()).is_err());
        assert!(
            Profile {
                profile_velocity: 0,
                ..Profile::default()
            }
            .validate()
            .is_err()
        );
        assert_eq!(Profile::default().bytes(), [20, 0, 0, 0, 30, 0, 0, 0]);
    }
}
