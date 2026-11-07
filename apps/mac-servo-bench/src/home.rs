use crate::{
    bus::Bus,
    model::{IDS, Profile, Sample},
    trace::Log,
};
use anyhow::{Result, bail, ensure};
use clap::Args;
use serde_json::json;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const CENTER: i32 = 2048;
const TARGET: [i32; 15] = [CENTER; 15];
const TOLERANCE_TICKS: i32 = 12; // Approximately 1.05 degrees.
const SETTLED_VELOCITY: f64 = 0.03; // rad/s; allows one raw velocity unit.
const SETTLED_SAMPLES: usize = 3;

#[derive(Args, Clone)]
pub struct Home {
    /// Maximum seconds to reach the center after submitting the goal.
    #[arg(long, default_value_t = 20.0)]
    timeout: f64,
    #[command(flatten)]
    pub profile: Profile,
}

impl Home {
    pub fn validate(&self) -> Result<()> {
        self.profile.validate()?;
        ensure!(
            self.timeout.is_finite() && (1.0..=300.0).contains(&self.timeout),
            "timeout must be 1..300 seconds"
        );
        Ok(())
    }
}

fn settled(sample: &Sample) -> bool {
    sample.ticks.iter().zip(sample.velocity).all(|(&p, v)| {
        (i64::from(p) - i64::from(CENTER)).abs() <= i64::from(TOLERANCE_TICKS)
            && v.abs() <= SETTLED_VELOCITY
    })
}

pub fn run(bus: &mut Bus, cfg: &Home, stop: &AtomicBool, log: &Log) -> Result<()> {
    ensure!(
        !stop.load(Ordering::Relaxed),
        "homing interrupted before sending center target"
    );
    log.emit(
        "home_plan",
        json!({"ids":IDS,"target_ticks":TARGET,"target_deg":([180.0;15]),
            "timeout_s":cfg.timeout,"tolerance_ticks":TOLERANCE_TICKS,
            "settled_velocity_rad_s":SETTLED_VELOCITY,"required_settled_samples":SETTLED_SAMPLES,
            "profile_velocity_raw":cfg.profile.profile_velocity,
            "profile_acceleration_raw":cfg.profile.profile_acceleration}),
        Some(format!(
            "HOME: IDs 1..15 -> 2048 ticks (180 deg); Profile Velocity={}, Acceleration={}; timeout={}s",
            cfg.profile.profile_velocity, cfg.profile.profile_acceleration, cfg.timeout
        )),
    );
    log.flush()?;
    ensure!(
        !stop.load(Ordering::Relaxed),
        "homing interrupted before sending center target"
    );
    // Submit once so that polling does not restart the servo's motion profile.
    bus.targets(&TARGET)?;
    let begin = Instant::now();
    let deadline = begin + Duration::from_secs_f64(cfg.timeout);
    let mut consecutive = 0;
    loop {
        ensure!(!stop.load(Ordering::Relaxed), "homing interrupted");
        let cycle = Instant::now();
        let sample = bus.sample()?;
        sample.check()?;
        ensure!(!stop.load(Ordering::Relaxed), "homing interrupted");
        consecutive = if settled(&sample) { consecutive + 1 } else { 0 };
        let errors = sample.ticks.map(|p| i64::from(p) - i64::from(CENTER));
        log.emit(
            "home_feedback",
            json!({"feedback":sample.json(),"error_ticks":errors,
                "motion_elapsed_s":begin.elapsed().as_secs_f64(),"settled_samples":consecutive}),
            Some(format!(
                "HOME {:.2}s position_ticks={:?}\nerror_ticks={errors:?}; settled={consecutive}/{SETTLED_SAMPLES}",
                begin.elapsed().as_secs_f64(), sample.ticks
            )),
        );
        log.flush()?;
        ensure!(!stop.load(Ordering::Relaxed), "homing interrupted");
        if Instant::now() >= deadline {
            bail!(
                "homing timed out after {:.1}s; not all 15 motors confirmed settled at 2048; last errors={errors:?}",
                cfg.timeout
            );
        }
        if consecutive >= SETTLED_SAMPLES {
            log.emit(
                "home_complete",
                json!({"success":true,"ids":IDS,"target_ticks":TARGET,
                    "position_ticks":sample.ticks,"error_ticks":errors,
                    "motion_elapsed_s":begin.elapsed().as_secs_f64()}),
                Some("HOME reached: all 15 motors settled within 12 ticks of 2048 (180 deg); turning torque off".into()),
            );
            return log.flush();
        }
        crate::sleep_until((cycle + Duration::from_millis(200)).min(deadline), stop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_every_motor_near_center_and_slow() {
        let mut sample = Sample::simulated([CENTER; 15]);
        assert!(settled(&sample));
        sample.ticks[14] += TOLERANCE_TICKS;
        assert!(settled(&sample));
        sample.ticks[14] += 1;
        assert!(!settled(&sample));
        sample.ticks[14] = CENTER;
        sample.velocity[14] = -0.1;
        assert!(!settled(&sample));
        sample.velocity[14] = 0.0;
        sample.ticks[0] = i32::MIN;
        assert!(!settled(&sample));
    }

    #[test]
    fn rejects_invalid_timeout_and_unlimited_profile() {
        let mut cfg = Home {
            timeout: 20.0,
            profile: Profile::default(),
        };
        assert!(cfg.validate().is_ok());
        for timeout in [0.0, 0.9, 301.0, f64::NAN, f64::INFINITY] {
            cfg.timeout = timeout;
            assert!(cfg.validate().is_err());
        }
        cfg.timeout = 20.0;
        cfg.profile.profile_velocity = 0;
        assert!(cfg.validate().is_err());
    }
}
