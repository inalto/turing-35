//! `turing-dashboard [--port COMx] [--interval MS] [--ticks N] [--quiet]`
//!
//! Draws the cyberpunk telemetry dashboard on the Turing 3.5" display and
//! refreshes it every `MS` milliseconds (default 1000, at least 200, the
//! shortest interval sysinfo can measure CPU load over), forever or for `N`
//! rendered frames. Whole-second intervals tick just after wall-clock second
//! boundaries. One line per tick is logged to stderr.
use std::time::Duration;

use turing_dashboard::dashboard::{interval_from_ms, run};
use turing_dashboard::telemetry::SysInfoSampler;

fn usage() -> ! {
    eprintln!("usage: turing-dashboard [--port COMx] [--interval MS=1000] [--ticks N] [--quiet]");
    eprintln!("  --interval is raised to at least 200 ms (sysinfo's minimum CPU refresh interval)");
    eprintln!("  --quiet    no per-tick or status output");
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
    let mut quiet = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" => port = Some(args.next().unwrap_or_else(|| usage())),
            "--interval" => interval = interval_from_ms(positive(args.next())),
            "--ticks" => ticks = Some(positive(args.next())),
            "--quiet" => quiet = true,
            _ => usage(),
        }
    }
    let mut sampler = SysInfoSampler::new();
    run(&mut sampler, port, interval, ticks, quiet)
}
