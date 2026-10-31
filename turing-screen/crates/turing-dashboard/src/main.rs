//! `turing-dashboard [--port COMx] [--flip] [--interval MS] [--ticks N] [--debug]`
//!
//! Draws the cyberpunk telemetry dashboard on the Turing 3.5" display in
//! portrait (`--flip` turns it upside down for the other mounting) and
//! refreshes it every `MS` milliseconds (default 1000, at least 200, the
//! shortest interval sysinfo can measure CPU load over), forever or for `N`
//! rendered frames. Whole-second intervals tick just after wall-clock second
//! boundaries. One line per tick is logged to stderr.
use std::time::Duration;

use turing_dashboard::dashboard::{interval_from_ms, run};
use turing_dashboard::telemetry::SysInfoSampler;

fn usage() -> ! {
    eprintln!("usage: turing-dashboard [--port COMx] [--flip] [--interval MS=1000] [--ticks N] [--debug]");
    eprintln!("  --flip     rotate the portrait image by 180 degrees (panel mounted cable-up)");
    eprintln!("  --interval is raised to at least 200 ms (sysinfo's minimum CPU refresh interval)");
    eprintln!("  --debug    log one line per tick and connection events to stderr (silent by default)");
    std::process::exit(2)
}

/// The next argument parsed as a positive integer, or exit with usage.
fn positive(arg: Option<String>) -> u64 {
    match arg.and_then(|s| s.parse::<u64>().ok()) {
        Some(n) if n > 0 => n,
        _ => usage(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut port: Option<String> = None;
    let mut interval = Duration::from_millis(1000);
    let mut ticks: Option<u64> = None;
    let mut debug = false;
    let mut flip = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" => port = Some(args.next().unwrap_or_else(|| usage())),
            "--interval" => interval = interval_from_ms(positive(args.next())),
            "--ticks" => ticks = Some(positive(args.next())),
            "--debug" => debug = true,
            "--flip" => flip = true,
            _ => usage(),
        }
    }
    let mut sampler = SysInfoSampler::new();
    run(&mut sampler, port, flip, interval, ticks, debug)
}
