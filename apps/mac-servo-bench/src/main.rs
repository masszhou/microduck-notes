mod bus;
mod home;
mod model;
mod trace;

use anyhow::{Result, anyhow, ensure};
use clap::{Args, Parser, Subcommand};
use model::{IDS, POLICY_IDS, Pattern, Profile, Sample, Wave};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use trace::Log;

#[derive(Parser)]
#[command(
    version,
    about = "15 XL330-M288 servos on OpenRB-150: raw bus + RL-shaped arrays"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    /// Explicit macOS callout port, e.g. /dev/cu.usbmodem1101. Otherwise detect one OpenRB.
    #[arg(long, global = true)]
    port: Option<String>,
    /// Current bus baud; does not change servo EEPROM.
    #[arg(long, global = true, default_value_t = 57600)]
    baud: u32,
    /// Hide packet hex on screen; the JSONL log still records every byte and packet.
    #[arg(long, global = true)]
    quiet_bus: bool,
    /// New JSONL file; existing files are never overwritten.
    #[arg(long, global = true)]
    log: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// List ports without opening them.
    Ports,
    /// Read model, configuration and feedback from IDs 1..15. No register writes.
    Probe,
    /// Run a slow wave or mirrored sweep on the REAL motors, then turn torque off.
    Run(Motion),
    /// Move all 15 REAL motors to 2048 (180 deg), verify arrival, then turn torque off.
    Home(home::Home),
    /// Same arrays with explicitly SIMULATED feedback; opens no serial port.
    Demo(Motion),
    /// Explicitly turn torque off for IDs 1..15, even if a device cannot be probed.
    Off,
}

#[derive(Args, Clone)]
struct Motion {
    /// wave: staggered phases/amplitudes; mirror: full amplitude, opposite odd/even IDs.
    #[arg(long, value_enum, default_value = "mirror")]
    pattern: Pattern,
    #[arg(long, default_value_t = 30.0)]
    duration: f64,
    #[arg(long, default_value_t = 5.0)]
    hz: f64,
    #[arg(long, default_value_t = 45.0)]
    amplitude_deg: f64,
    #[arg(long, default_value_t = 8.0)]
    period: f64,
    #[command(flatten)]
    profile: Profile,
}

fn ports() -> Result<Vec<serialport::SerialPortInfo>> {
    Ok(serialport::available_ports()?)
}

fn resolve_port(explicit: &Option<String>) -> Result<String> {
    if let Some(p) = explicit {
        return Ok(p.clone());
    }
    let candidates: Vec<_> = ports()?
        .into_iter()
        .filter(|p| {
            let serialport::SerialPortType::UsbPort(u) = &p.port_type else {
                return false;
            };
            let label = format!(
                "{} {}",
                u.manufacturer.as_deref().unwrap_or(""),
                u.product.as_deref().unwrap_or("")
            )
            .to_lowercase();
            p.port_name.starts_with("/dev/cu.")
                && (label.contains("openrb") || label.contains("robotis"))
        })
        .collect();
    ensure!(
        candidates.len() == 1,
        "found {} OpenRB candidates; use `ports`, then specify --port /dev/cu.usbmodem...",
        candidates.len()
    );
    Ok(candidates[0].port_name.clone())
}

fn sleep_until(deadline: Instant, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        thread::sleep(left.min(Duration::from_millis(20)));
    }
}

fn formatted(values: impl IntoIterator<Item = f64>) -> String {
    format!(
        "[{}]",
        values
            .into_iter()
            .map(|x| format!("{x:7.3}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

struct CycleInfo {
    tick: u64,
    t: f64,
    simulated: bool,
}

fn show_cycle(
    log: &Log,
    wave: &Wave,
    sample: &Sample,
    previous: &[f32; 14],
    target: &[i32; 15],
    info: CycleInfo,
) -> [f32; 14] {
    let CycleInfo { tick, t, simulated } = info;
    let obs = wave.observation(sample, previous);
    let action = wave.action(target);
    let target_deg = target.map(|p| p as f64 * 360.0 / 4096.0);
    let error_deg: [f64; 15] =
        std::array::from_fn(|i| (target[i] - sample.ticks[i]) as f64 * 360.0 / 4096.0);
    let display = format!(
        "tick={tick} t={t:.3}s source={} IMU=SYNTHETIC command=ZERO action=DUMMY_{}\n\
        present_deg[15] = {}\n\
        velocity_rad_s  = {}\n\
        current_ma      = {}  sum_abs={:.0}mA\n\
        obs.gyro[0:3]   = {} (synthetic)\n\
        obs.gravity[3:6]= {} (synthetic)\n\
        obs.q_rel[6:20] = {}\n\
        obs.dq[20:34]   = {}\n\
        obs.prev[34:48] = {}\n\
        obs.cmd[48:61]  = {} (zero command)\n\
        observation[61]= {}\n\
        action[14]     = {}\n\
        target_deg[15] = {} (ID 10 mouth included)\n\
        new_target_minus_present_deg = {}",
        if simulated { "SIMULATED" } else { "HARDWARE" },
        wave.pattern.name().to_uppercase(),
        formatted(sample.ticks.map(|p| p as f64 * 360.0 / 4096.0)),
        formatted(sample.velocity),
        formatted(sample.current_ma),
        sample.current_ma.iter().map(|v| v.abs()).sum::<f64>(),
        formatted(obs[0..3].iter().map(|v| *v as f64)),
        formatted(obs[3..6].iter().map(|v| *v as f64)),
        formatted(obs[6..20].iter().map(|v| *v as f64)),
        formatted(obs[20..34].iter().map(|v| *v as f64)),
        formatted(obs[34..48].iter().map(|v| *v as f64)),
        formatted(obs[48..61].iter().map(|v| *v as f64)),
        formatted(obs.iter().map(|v| *v as f64)),
        formatted(action.map(|v| v as f64)),
        formatted(target_deg),
        formatted(error_deg)
    );
    log.emit("cycle_plan", json!({"tick":tick,"motion_elapsed_s":t,"feedback_source":if simulated {"simulated"} else {"hardware"},
        "imu_source":"synthetic_upright","command_source":"zero","action_source":format!("dummy_{}",wave.pattern.name()),"policy_ready":false,
        "feedback":sample.json(),"observation":obs.as_slice(),"action":action,"previous_action":previous,"action_ids":POLICY_IDS,
        "target_ids":IDS,"target_ticks":target,"target_deg":target_deg,"mouth_id":10,"mouth_target_ticks":target[9],
        "home_ticks":wave.home,"action_scale_rad":wave.scale}), Some(display));
    action
}

fn motion_loop(
    mut bus: Option<&mut bus::Bus>,
    start: [i32; 15],
    cfg: &Motion,
    stop: &AtomicBool,
    log: &Log,
) -> Result<()> {
    let wave = Wave::new(start, cfg.amplitude_deg, cfg.period, cfg.pattern);
    log.emit(
        "motion_parameters",
        json!({"pattern":cfg.pattern.name(),"amplitude_deg":cfg.amplitude_deg,
        "period_s":cfg.period,"requested_duration_s":cfg.duration,"hz":cfg.hz,"fade_in_s":4.0,
        "profile_velocity_raw":cfg.profile.profile_velocity,"profile_acceleration_raw":cfg.profile.profile_acceleration,
        "profile_velocity_deg_s":cfg.profile.velocity_deg_s(),"profile_acceleration_deg_s2":cfg.profile.acceleration_deg_s2()}),
        Some(format!(
            "Pattern={} amplitude=+/-{} deg period={}s duration={}s; full amplitude after 4s\nProfile Velocity={} raw ({:.2} deg/s), Profile Acceleration={} raw ({:.2} deg/s^2)",
            cfg.pattern.name(),
            cfg.amplitude_deg,
            cfg.period,
            cfg.duration,
            cfg.profile.profile_velocity, cfg.profile.velocity_deg_s(), cfg.profile.profile_acceleration, cfg.profile.acceleration_deg_s2()
        )),
    );
    let mut previous = [0.0; 14];
    let mut simulated = Sample::simulated(start);
    let begin = Instant::now();
    let interval = Duration::from_secs_f64(1.0 / cfg.hz);
    let mut tick = 0u64;
    let mut overruns = 0u64;
    while !stop.load(Ordering::Relaxed) && begin.elapsed().as_secs_f64() < cfg.duration {
        let loop_start = Instant::now();
        let sample = if let Some(b) = bus.as_deref_mut() {
            b.sample()?
        } else {
            simulated.clone()
        };
        sample.check()?;
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let t = begin.elapsed().as_secs_f64();
        let target = wave.target(t);
        let action = show_cycle(
            log,
            &wave,
            &sample,
            &previous,
            &target,
            CycleInfo {
                tick,
                t,
                simulated: bus.is_none(),
            },
        );
        log.flush()?; // An unwritable log or broken stdout stops motion before sending a new target.
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if let Some(b) = bus.as_deref_mut() {
            b.targets(&target)?;
        } else {
            let old = simulated.ticks;
            simulated.ticks = std::array::from_fn(|i| {
                (old[i] as f64 + 0.7 * (target[i] - old[i]) as f64).round() as i32
            });
            simulated.velocity = std::array::from_fn(|i| {
                (simulated.ticks[i] - old[i]) as f64 * model::TICK_RAD * cfg.hz
            });
        }
        previous = action;
        let cost = loop_start.elapsed();
        let overrun = cost > interval;
        overruns += u64::from(overrun);
        log.emit(
            "cycle_complete",
            json!({"tick":tick,"work_ms":cost.as_secs_f64()*1000.0,"overrun":overrun,
            "goal_packet_submitted":bus.is_some(),"goal_acknowledged":false}),
            Some(format!(
                "cycle work={:.1}ms budget={:.1}ms overrun={overrun}; Sync Write has no ACK",
                cost.as_secs_f64() * 1000.0,
                interval.as_secs_f64() * 1000.0
            )),
        );
        log.flush()?;
        tick += 1;
        // Do not burst to catch up if the terminal or bus is slow.
        sleep_until(loop_start + interval, stop);
    }
    log.emit("run_summary", json!({"cycles":tick,"motion_elapsed_s":begin.elapsed().as_secs_f64(),"overruns":overruns,"interrupted":stop.load(Ordering::Relaxed)}),
        Some(format!("Finished {tick} cycles, {overruns} overruns, elapsed {:.2}s",begin.elapsed().as_secs_f64())));
    Ok(())
}

fn execute(cli: &Cli, stop: &AtomicBool, log: &Log) -> Result<()> {
    match &cli.command {
        Command::Demo(cfg) => {
            model::validate_motion(
                cfg.hz,
                cfg.duration,
                cfg.amplitude_deg,
                cfg.period,
                cli.baud,
                cfg.profile,
            )?;
            log.emit(
                "mode",
                json!({"mode":"demo","opens_serial":false}),
                Some("DEMO: all feedback simulated; no hardware port opened".into()),
            );
            motion_loop(None, [2048; 15], cfg, stop, log)
        }
        Command::Run(_) | Command::Home(_) | Command::Probe | Command::Off => {
            if let Command::Run(cfg) = &cli.command {
                model::validate_motion(
                    cfg.hz,
                    cfg.duration,
                    cfg.amplitude_deg,
                    cfg.period,
                    cli.baud,
                    cfg.profile,
                )?;
            }
            if let Command::Home(cfg) = &cli.command {
                cfg.validate()?;
            }
            let port = resolve_port(&cli.port)?;
            log.emit("mode",json!({"mode":match cli.command {Command::Probe=>"probe",Command::Off=>"off",Command::Home(_)=>"home",_=>"run"},"port":port,"baud":cli.baud}),Some(format!("OpenRB port={port} baud={} IDs=1..15",cli.baud)));
            let mut bus = bus::Bus::open(&port, cli.baud, log.clone())?;
            // An emergency off must reach the other motors even if one device is missing.
            if matches!(cli.command, Command::Off) {
                return bus.off();
            }
            bus.probe(stop)?;
            match &cli.command {
                Command::Probe => {
                    let s = bus.sample()?;
                    log.emit("probe_feedback",s.json(),Some(format!("All 15 XL330-M288 detected. No register writes.\nvoltage_v={}\ntemperature_c={:?}\ncurrent_ma={}",formatted(s.volts),s.temperature,formatted(s.current_ma))));
                    s.check()
                }
                Command::Off => bus.off(),
                Command::Run(_) | Command::Home(_) => {
                    let result = match &cli.command {
                        Command::Run(cfg) => bus
                            .start(stop, cfg.profile)
                            .and_then(|start| motion_loop(Some(&mut bus), start, cfg, stop, log)),
                        Command::Home(cfg) => bus
                            .start(stop, cfg.profile)
                            .and_then(|_| home::run(&mut bus, cfg, stop, log)),
                        _ => unreachable!(),
                    };
                    let cleanup = bus.shutdown(true);
                    match (result, cleanup) {
                        (Err(e), Err(c)) => Err(anyhow!("{e:#}; shutdown: {c:#}")),
                        (Err(e), _) => Err(e),
                        (_, Err(e)) => Err(e),
                        _ => Ok(()),
                    }
                }
                _ => unreachable!(),
            }
        }
        Command::Ports => unreachable!(),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if matches!(cli.command, Command::Ports) {
        for p in ports()? {
            println!("{}  {:?}", p.port_name, p.port_type);
        }
        return Ok(());
    }
    ensure!(
        [57600, 115200, 1000000].contains(&cli.baud),
        "supported baud: 57600, 115200, 1000000"
    );
    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, stop.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, stop.clone())?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let path = cli
        .log
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("logs/{stamp}-{}.jsonl", std::process::id())));
    let log = Log::new(&path, cli.quiet_bus)?;
    log.emit("session",json!({"schema_version":1,"unix_ms":stamp,"ids":IDS,"action_ids":POLICY_IDS,"mouth_id":10,
        "observation_layout":{"gyro":[0,3],"gravity":[3,6],"joint_position_relative_to_bench_home":[6,20],"joint_velocity":[20,34],"previous_action":[34,48],"command":[48,61]},
        "policy_ready":false,"imu_source":"synthetic_upright","bench_mapping":"IDs 1..15 in microduck logical slot order; not calibrated robot joints"}),
        Some(format!("Log: {}\nID order: {IDS:?}\nAction order: {POLICY_IDS:?}; ID 10 is separate mouth demo slot\nCtrl+C stops; motion run attempts torque OFF on exit.",path.display())));
    let result = execute(&cli, &stop, &log);
    if let Err(e) = &result {
        log.emit(
            "error",
            json!({"message":format!("{e:#}")}),
            Some(format!("ERROR: {e:#}")),
        );
    }
    let flushed = log.flush();
    result.and(flushed)
}
