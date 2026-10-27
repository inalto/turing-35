//! System telemetry: a sampler producing [`Snapshot`]s, a fixed-size history
//! [`Ring`], and display formatters for the dashboard.
//!
//! Sizes use 1024-based units but are printed as `KB`, `MB` and `GB`.

use std::collections::VecDeque;
use std::time::Instant;

use chrono::{DateTime, Local};
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::Nvml;
use sysinfo::{Components, Disks, MemoryRefreshKind, Networks, System};

const KIB: f64 = 1024.0;
const MIB: f64 = KIB * 1024.0;
const GIB: f64 = MIB * 1024.0;

/// Statistics of the primary GPU.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuStats {
    /// Marketing name of the GPU.
    pub name: String,
    /// Core utilisation, `0..=100`.
    pub load: f32,
    /// Core temperature in degrees Celsius, if readable.
    pub temp_c: Option<f32>,
    /// Video memory in use, in bytes.
    pub vram_used: u64,
    /// Total video memory, in bytes.
    pub vram_total: u64,
}

/// One point-in-time reading of the whole machine.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// CPU brand string.
    pub cpu_name: String,
    /// Overall CPU utilisation, `0..=100`.
    pub cpu_load: f32,
    /// Per-core utilisation, each `0..=100`.
    pub cpu_cores: Vec<f32>,
    /// CPU temperature in degrees Celsius, if a sensor is exposed.
    pub cpu_temp_c: Option<f32>,
    /// Primary GPU statistics, if NVML is available.
    pub gpu: Option<GpuStats>,
    /// Used physical memory, in bytes.
    pub mem_used: u64,
    /// Total physical memory, in bytes.
    pub mem_total: u64,
    /// Label of the sampled disk (`"C:"` or `"/"`).
    pub disk_label: String,
    /// Used space on the sampled disk, in bytes.
    pub disk_used: u64,
    /// Total space on the sampled disk, in bytes.
    pub disk_total: u64,
    /// Download throughput in bytes per second.
    pub net_down_bps: f64,
    /// Upload throughput in bytes per second.
    pub net_up_bps: f64,
    /// Local wall-clock time of the sample.
    pub time: DateTime<Local>,
}

/// A source of [`Snapshot`]s.
pub trait Sampler {
    /// Take a new reading.
    fn sample(&mut self) -> Snapshot;
}

/// Per-interface `(name, rx, tx)` byte totals.
type NetTotals = Vec<(String, u64, u64)>;

/// Network state at the previous sample: the chosen interface and the
/// totals of every interface.
struct NetState {
    iface: Option<String>,
    totals: NetTotals,
    at: Instant,
}

/// [`Sampler`] backed by `sysinfo` and, when available, NVML.
pub struct SysInfoSampler {
    sys: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    nvml: Option<Nvml>,
    net: Option<NetState>,
}

impl SysInfoSampler {
    /// Create a sampler and perform the first CPU refresh.
    ///
    /// Never panics: a missing NVML, disk or network interface degrades to
    /// `None` or zero values.
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory_specifics(MemoryRefreshKind::everything());
        Self {
            sys,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            nvml: Nvml::init().ok(),
            net: None,
        }
    }

    fn gpu(&self) -> Option<GpuStats> {
        let dev = self.nvml.as_ref()?.device_by_index(0).ok()?;
        let mem = dev.memory_info().ok()?;
        Some(GpuStats {
            name: dev.name().unwrap_or_else(|_| "GPU".to_string()),
            load: dev
                .utilization_rates()
                .map(|u| (u.gpu as f32).clamp(0.0, 100.0))
                .unwrap_or(0.0),
            temp_c: dev
                .temperature(TemperatureSensor::Gpu)
                .ok()
                .map(|t| t as f32),
            vram_used: mem.used,
            vram_total: mem.total,
        })
    }

    fn cpu_temp(&self) -> Option<f32> {
        self.components
            .iter()
            .filter(|c| {
                let l = c.label().to_ascii_lowercase();
                ["cpu", "tctl", "tdie", "package", "core"]
                    .iter()
                    .any(|k| l.contains(k))
            })
            .filter_map(|c| c.temperature())
            .filter(|t| t.is_finite() && *t > 0.0)
            .reduce(f32::max)
    }

    /// `(label, used, total)` of the system disk, else the first disk.
    fn disk(&self) -> (String, u64, u64) {
        let list = self.disks.list();
        let system = list
            .iter()
            .find(|d| is_system_mount(&d.mount_point().to_string_lossy()));
        let Some(d) = system.or_else(|| list.first()) else {
            return (String::new(), 0, 0);
        };
        let mount = d.mount_point().to_string_lossy();
        let trimmed = mount.trim_end_matches(['\\', '/']);
        let label = if trimmed.is_empty() { "/" } else { trimmed };
        let total = d.total_space();
        (
            label.to_string(),
            total.saturating_sub(d.available_space()),
            total,
        )
    }

    /// `(down, up)` in bytes per second since the previous call, for the
    /// interface chosen by [`select_interface`].
    fn throughput(&mut self) -> (f64, f64) {
        let now = Instant::now();
        let cur: NetTotals = self
            .networks
            .iter()
            .map(|(name, n)| (name.clone(), n.total_received(), n.total_transmitted()))
            .collect();
        let (prev_choice, prev_totals, prev_at) = match &self.net {
            Some(p) => (p.iface.as_deref(), p.totals.as_slice(), Some(p.at)),
            None => (None, &[][..], None),
        };
        let iface = select_interface(prev_choice, prev_totals, &cur);
        let rates = iface
            .as_deref()
            .and_then(|name| {
                let (_, rx, tx) = cur.iter().find(|(n, ..)| n == name)?;
                let (_, prx, ptx) = prev_totals.iter().find(|(n, ..)| n == name)?;
                let secs = now.duration_since(prev_at?).as_secs_f64();
                (secs > 0.0).then(|| {
                    (
                        rx.saturating_sub(*prx) as f64 / secs,
                        tx.saturating_sub(*ptx) as f64 / secs,
                    )
                })
            })
            .unwrap_or((0.0, 0.0));
        self.net = Some(NetState {
            iface,
            totals: cur,
            at: now,
        });
        rates
    }
}

impl Default for SysInfoSampler {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether `name` is a loopback interface (`Loopback Pseudo-Interface 1`
/// on Windows, `lo`/`lo0` elsewhere).
fn is_loopback(name: &str) -> bool {
    name.contains("Loopback") || name.starts_with("lo")
}

/// Choose the interface to report: the non-loopback interface whose
/// `rx + tx` grew the most between `prev` and `cur` totals.
///
/// The previous choice is kept on a tie or when nothing moved. Without a
/// usable previous choice and with no traffic, the interface with the
/// largest lifetime total wins. Remaining ties go to the smallest name so
/// the result never depends on enumeration order.
fn select_interface(
    prev_choice: Option<&str>,
    prev: &[(String, u64, u64)],
    cur: &[(String, u64, u64)],
) -> Option<String> {
    let delta = |name: &str, rx: u64, tx: u64| {
        prev.iter()
            .find(|(n, ..)| n == name)
            .map(|(_, prx, ptx)| {
                rx.saturating_sub(*prx)
                    .saturating_add(tx.saturating_sub(*ptx))
            })
            .unwrap_or(0)
    };
    let mut candidates: Vec<(&str, u64, u64)> = cur
        .iter()
        .filter(|(n, ..)| !is_loopback(n))
        .map(|(n, rx, tx)| (n.as_str(), delta(n, *rx, *tx), rx.saturating_add(*tx)))
        .collect();
    candidates.sort_by(|a, b| a.0.cmp(b.0));
    let best = candidates.iter().map(|c| c.1).max()?;
    let previous = prev_choice.and_then(|p| candidates.iter().find(|c| c.0 == p));
    let chosen = match previous {
        Some(p) if p.1 == best => p.0,
        _ if best > 0 => candidates.iter().find(|c| c.1 == best)?.0,
        _ => {
            let top = candidates.iter().map(|c| c.2).max()?;
            candidates.iter().find(|c| c.2 == top)?.0
        }
    };
    Some(chosen.to_string())
}

/// Whether `mount` is the system drive (`C:\` on Windows, `/` elsewhere).
fn is_system_mount(mount: &str) -> bool {
    if cfg!(windows) {
        mount.to_ascii_uppercase().starts_with("C:")
    } else {
        mount == "/"
    }
}

impl Sampler for SysInfoSampler {
    fn sample(&mut self) -> Snapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.disks.refresh(true);
        self.networks.refresh(true);
        self.components.refresh(true);
        let (net_down_bps, net_up_bps) = self.throughput();
        let (disk_label, disk_used, disk_total) = self.disk();
        let mem_total = self.sys.total_memory();
        Snapshot {
            cpu_name: self
                .sys
                .cpus()
                .first()
                .map(|c| c.brand().trim().to_string())
                .unwrap_or_default(),
            cpu_load: self.sys.global_cpu_usage().clamp(0.0, 100.0),
            cpu_cores: self
                .sys
                .cpus()
                .iter()
                .map(|c| c.cpu_usage().clamp(0.0, 100.0))
                .collect(),
            cpu_temp_c: self.cpu_temp(),
            gpu: self.gpu(),
            mem_used: self.sys.used_memory().min(mem_total),
            mem_total,
            disk_label,
            disk_used,
            disk_total,
            net_down_bps,
            net_up_bps,
            time: Local::now(),
        }
    }
}

/// Fixed-capacity history of the last [`Ring::LEN`] samples.
#[derive(Debug, Clone, Default)]
pub struct Ring {
    buf: VecDeque<f32>,
}

impl Ring {
    /// Number of samples retained.
    pub const LEN: usize = 60;

    /// Create an empty ring.
    pub fn new() -> Self {
        Self {
            buf: VecDeque::with_capacity(Self::LEN),
        }
    }

    /// Append a sample, dropping the oldest when full.
    pub fn push(&mut self, v: f32) {
        if self.buf.len() == Self::LEN {
            self.buf.pop_front();
        }
        self.buf.push_back(v);
    }

    /// Number of samples currently stored.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Whether no samples are stored.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Iterate samples from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = f32> + '_ {
        self.buf.iter().copied()
    }

    /// Largest stored sample, or `0.0` when empty.
    pub fn max(&self) -> f32 {
        self.buf.iter().copied().fold(0.0, f32::max)
    }
}

/// Format a throughput, e.g. `"0 B/s"`, `"212 KB/s"`, `"1.8 MB/s"`, `"1.2 GB/s"`.
pub fn fmt_rate(bytes_per_s: f64) -> String {
    let b = if bytes_per_s.is_finite() {
        bytes_per_s.max(0.0)
    } else {
        0.0
    };
    if b < KIB {
        format!("{b:.0} B/s")
    } else if b < MIB {
        format!("{:.0} KB/s", b / KIB)
    } else if b < GIB {
        format!("{:.1} MB/s", b / MIB)
    } else {
        format!("{:.1} GB/s", b / GIB)
    }
}

/// Format a byte count as GiB with one decimal, number only (e.g. `"12.4"`).
pub fn fmt_gb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / GIB)
}

/// Format a percentage rounded half up and clamped to `0..=100` (e.g. `"37%"`).
pub fn fmt_pct(v: f32) -> String {
    let v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 100.0) };
    format!("{}%", (v + 0.5).floor() as u32)
}

/// Format a temperature (e.g. `"61°C"`), or `"--"` when unknown.
pub fn fmt_temp(t: Option<f32>) -> String {
    match t {
        Some(t) if t.is_finite() => format!("{}\u{b0}C", t.round() as i32),
        _ => "--".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::select_interface;

    fn t(list: &[(&str, u64, u64)]) -> Vec<(String, u64, u64)> {
        list.iter()
            .map(|&(n, rx, tx)| (n.to_string(), rx, tx))
            .collect()
    }

    #[test]
    fn picks_the_largest_delta_not_the_largest_total() {
        let prev = t(&[("Ethernet", 1_000_000, 0), ("Wi-Fi", 10, 10)]);
        let cur = t(&[("Ethernet", 1_000_100, 0), ("Wi-Fi", 5_010, 10)]);
        assert_eq!(
            select_interface(Some("Ethernet"), &prev, &cur).as_deref(),
            Some("Wi-Fi")
        );
    }

    #[test]
    fn skips_loopback_interfaces() {
        let prev = t(&[
            ("Loopback Pseudo-Interface 1", 0, 0),
            ("lo", 0, 0),
            ("Ethernet", 0, 0),
        ]);
        let cur = t(&[
            ("Loopback Pseudo-Interface 1", 9_000, 9_000),
            ("lo", 8_000, 0),
            ("Ethernet", 1, 0),
        ]);
        assert_eq!(
            select_interface(None, &prev, &cur).as_deref(),
            Some("Ethernet")
        );
    }

    #[test]
    fn keeps_the_previous_choice_on_a_tie() {
        let prev = t(&[("A", 0, 0), ("B", 0, 0)]);
        let cur = t(&[("A", 50, 50), ("B", 100, 0)]);
        assert_eq!(
            select_interface(Some("B"), &prev, &cur).as_deref(),
            Some("B")
        );
        assert_eq!(
            select_interface(Some("A"), &prev, &cur).as_deref(),
            Some("A")
        );
    }

    #[test]
    fn keeps_the_previous_choice_when_all_deltas_are_zero() {
        let prev = t(&[("A", 900, 0), ("B", 5, 0)]);
        let cur = t(&[("A", 900, 0), ("B", 5, 0)]);
        assert_eq!(
            select_interface(Some("B"), &prev, &cur).as_deref(),
            Some("B")
        );
    }

    #[test]
    fn first_sample_falls_back_to_the_largest_total() {
        let cur = t(&[("lo", 1 << 40, 0), ("A", 10, 0), ("B", 900, 0)]);
        assert_eq!(select_interface(None, &[], &cur).as_deref(), Some("B"));
    }

    #[test]
    fn no_usable_interface_selects_nothing() {
        assert_eq!(select_interface(None, &[], &[]), None);
        assert_eq!(select_interface(Some("A"), &[], &t(&[("lo", 1, 1)])), None);
    }
}
