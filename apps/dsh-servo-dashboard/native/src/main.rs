mod engine;
mod model;
mod wire;
use engine::Engine;
use serde_json::Value;
use std::{
    collections::HashSet,
    io::{self, BufRead, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

fn main() {
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        if let Err(e) = signal_hook::flag::register(signal, stop.clone()) {
            eprintln!("signal registration failed: {e}");
            std::process::exit(1);
        }
    }
    let (tx, rx) = mpsc::sync_channel::<Option<Vec<u8>>>(256);
    thread::spawn(move || {
        let mut input = io::stdin().lock();
        let mut oversized = false;
        loop {
            let mut line = Vec::new();
            let result = std::io::Read::take(&mut input, 16385).read_until(b'\n', &mut line);
            match result {
                Ok(0) | Err(_) => {
                    let _ = tx.send(None);
                    break;
                }
                Ok(_) => {
                    let ends = line.last() == Some(&b'\n');
                    if line.len() > 16384 || oversized {
                        if !oversized && tx.send(Some(Vec::new())).is_err() {
                            break;
                        }
                        oversized = !ends;
                        continue;
                    }
                    if tx.send(Some(line)).is_err() {
                        break;
                    }
                }
            }
        }
    });
    // stdout backpressure must not block lease or safety handling.
    let (out_tx, out_rx) = mpsc::sync_channel::<Value>(128);
    let broken = Arc::new(AtomicBool::new(false));
    let output_broken = broken.clone();
    let (done_tx, done_rx) = mpsc::channel();
    let writer = thread::spawn(move || {
        let mut output = io::stdout().lock();
        for v in out_rx {
            if writeln!(output, "{v}")
                .and_then(|_| output.flush())
                .is_err()
            {
                output_broken.store(true, Ordering::Relaxed);
                break;
            }
        }
        let _ = done_tx.send(());
    });
    let mut engine = Engine::default();
    let mut seen = HashSet::new();
    let mut exiting = false;
    while !exiting && !stop.load(Ordering::Relaxed) && !broken.load(Ordering::Relaxed) {
        // Bound command processing so a flooded stdin cannot starve safety ticks.
        for _ in 0..1 {
            match rx.try_recv() {
                Ok(Some(bytes)) => {
                    let parsed = serde_json::from_slice::<Value>(&bytes);
                    match parsed {
                        Err(e) => {
                            engine.failure(Value::Null, format!("invalid/oversized JSON line: {e}"))
                        }
                        Ok(v) => {
                            let request = v.get("requestId").cloned().unwrap_or(Value::Null);
                            match model::parse(&v) {
                                Err(e) => engine.failure(request, e.to_string()),
                                Ok(action) => {
                                    if seen.len() >= 100000 {
                                        engine.failure(
                                            request,
                                            "request budget exhausted; reconnect worker".into(),
                                        );
                                        exiting = true;
                                        break;
                                    }
                                    if !seen.insert(request.to_string()) {
                                        engine.failure(request,"duplicate requestId refused; never automatically resend unknown motion".into());
                                    } else {
                                        engine.handle(request, action);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => {
                    exiting = true;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        if !exiting {
            engine.tick();
        }
        for v in engine.outputs.drain(..) {
            if out_tx.try_send(v).is_err() {
                broken.store(true, Ordering::Relaxed);
                break;
            }
        }
        if !exiting {
            thread::park_timeout(Duration::from_millis(2));
        }
    }
    let clean = engine.shutdown();
    for v in engine.outputs.drain(..) {
        let _ = out_tx.try_send(v);
    }
    drop(out_tx);
    // A blocked pipe cannot delay cleanup; only join a writer that already exited.
    if done_rx.recv_timeout(Duration::from_millis(250)).is_ok() {
        let _ = writer.join();
    }
    if !clean {
        eprintln!(
            "cleanup incomplete; touched motors may remain energized; disconnect external power"
        );
        std::process::exit(2);
    }
    if broken.load(Ordering::Relaxed) {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests;
