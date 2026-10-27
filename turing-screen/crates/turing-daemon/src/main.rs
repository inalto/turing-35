//! `turing-daemon [--listen ADDR] [--port COMx] [--orientation NAME]`
//!
//! Finds the Turing 3.5" display, wakes it, and serves the HTTP API.
use turing_daemon::{router, spawn_renderer, DeviceInfo};
use turing_device::{discover, Display};
use turing_proto::Orientation;

fn usage() -> ! {
    eprintln!("usage: turing-daemon [--listen 127.0.0.1:8686] [--port COMx] [--orientation landscape|portrait|landscape_flipped|portrait_flipped]");
    std::process::exit(2)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listen = "127.0.0.1:8686".to_string();
    let mut port: Option<String> = None;
    let mut orientation = Orientation::Landscape;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--listen" => listen = args.next().unwrap_or_else(|| usage()),
            "--port" => port = Some(args.next().unwrap_or_else(|| usage())),
            "--orientation" => {
                orientation = match args.next().unwrap_or_else(|| usage()).as_str() {
                    "landscape" => Orientation::Landscape,
                    "landscape_flipped" => Orientation::LandscapeFlipped,
                    "portrait" => Orientation::Portrait,
                    "portrait_flipped" => Orientation::PortraitFlipped,
                    _ => usage(),
                }
            }
            _ => usage(),
        }
    }

    let found = discover()?;
    let candidate = match &port {
        Some(p) => found.iter().find(|c| &c.port == p).cloned().unwrap_or_else(|| turing_device::Candidate {
            port: p.clone(),
            pid: 0,
            serial: None,
        }),
        None => found.first().cloned().ok_or("no Turing display found (use --port to force one)")?,
    };
    eprintln!("display: {} ({})", candidate.port, candidate.serial.as_deref().unwrap_or("unknown serial"));

    let display = Display::open(&candidate.port)?;
    let info = DeviceInfo { port: candidate.port.clone(), serial: candidate.serial.clone() };
    let handle = spawn_renderer(display, info, orientation)?;

    let listener = tokio::net::TcpListener::bind(&listen).await?;
    eprintln!("listening on http://{listen}  (GET /v1/device)");
    axum::serve(listener, router(handle)).await?;
    Ok(())
}
