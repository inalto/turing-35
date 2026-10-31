//! `cargo run -p turing-dashboard --example preview -- [out.png]`
//!
//! Renders the dashboard for a made-up snapshot and writes it as a PNG
//! (default `dashboard_preview.png`), to check the layout without a panel.
use chrono::Local;
use turing_dashboard::dashboard::Dashboard;
use turing_dashboard::telemetry::{GpuStats, Snapshot};
use turing_dashboard::theme::{HEIGHT, WIDTH};

const GIB: u64 = 1 << 30;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args().nth(1).unwrap_or_else(|| "dashboard_preview.png".into());
    let mut s = Snapshot {
        cpu_name: "AMD Ryzen 7 5700G with Radeon Graphics".into(),
        cpu_load: 37.0,
        cpu_cores: (0..16).map(|i| (i * 6) as f32).collect(),
        cpu_temp_c: Some(61.0),
        gpu: Some(GpuStats {
            name: "NVIDIA GeForce GTX 1660 SUPER".into(),
            load: 12.0,
            temp_c: Some(48.0),
            vram_used: 2 * GIB,
            vram_total: 6 * GIB,
        }),
        mem_used: 12 * GIB,
        mem_total: 31 * GIB,
        disk_label: "C:".into(),
        disk_used: 412 * GIB,
        disk_total: 931 * GIB,
        net_down_bps: 1.8 * 1024.0 * 1024.0,
        net_up_bps: 212.0 * 1024.0,
        time: Local::now(),
    };
    let mut dash = Dashboard::new(&s);
    // A few ticks of varying traffic so the sparklines have a shape.
    for k in 0..40u32 {
        s.time += chrono::Duration::seconds(1);
        s.net_down_bps = (1.0 + (k as f64 * 0.4).sin().abs() * 3.0) * 1024.0 * 1024.0;
        s.net_up_bps = (0.2 + (k as f64 * 0.7).cos().abs()) * 1024.0 * 1024.0;
        dash.render(&s);
    }
    image::save_buffer(&out, dash.canvas.as_rgb(), u32::from(WIDTH), u32::from(HEIGHT), image::ColorType::Rgb8)?;
    println!("wrote {out}");
    Ok(())
}
