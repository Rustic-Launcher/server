//! Timings for the Rustic runtime: how much time each plugin spends in each event handler,
//! command, task and lifecycle call.
//!
//! * Collection can be switched on and off at runtime ([`Timings::set_enabled`]). When off, the
//!   only cost on the hot path is one relaxed atomic load ([`Timings::start`] returns `None`
//!   without reading the clock).
//! * For every `(plugin, kind, name)` section the collector keeps the count, total, max and a
//!   cheap log-linear [`Histogram`] (8 sub-buckets per power of two, about 6% precision) from
//!   which p50/p95/p99 are read.
//! * [`Timings::report`] snapshots everything into a serialisable [`TimingsReport`], rendered as
//!   a text table by [`TimingsReport::to_text`] (for `/rustic timings`) or exported as JSON
//!   into `<server>/timings/<timestamp>.json` by [`Timings::export_json`].
//!
//! The collector is `Send + Sync` (async tasks run on other threads and record too).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// What a timed section is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SectionKind {
    /// An event handler call; the name is the event name ("player-join").
    Event,
    /// A command execution; the name is the command name.
    Command,
    /// A tab completion call; the name is the command name.
    Completion,
    /// A main-thread scheduled task; the name is `timer#<id>` for repeating tasks and
    /// `run-later` for one-shot tasks (aggregated so the key set stays bounded).
    Task,
    /// A task run in the async instance (same naming as [`SectionKind::Task`]).
    AsyncTask,
    /// A `post-to-main` message delivered to `on-message`; the name is the topic.
    Message,
    /// `on-load`, `on-enable`, `on-disable`.
    Lifecycle,
    /// A call into a plugin made on behalf of another (services).
    Service,
}

impl SectionKind {
    /// Every kind, in report order.
    pub const ALL: [SectionKind; 8] = [
        SectionKind::Event,
        SectionKind::Command,
        SectionKind::Completion,
        SectionKind::Task,
        SectionKind::AsyncTask,
        SectionKind::Message,
        SectionKind::Lifecycle,
        SectionKind::Service,
    ];

    /// Short lowercase label.
    pub fn label(self) -> &'static str {
        match self {
            SectionKind::Event => "event",
            SectionKind::Command => "command",
            SectionKind::Completion => "completion",
            SectionKind::Task => "task",
            SectionKind::AsyncTask => "async-task",
            SectionKind::Message => "message",
            SectionKind::Lifecycle => "lifecycle",
            SectionKind::Service => "service",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

const LINEAR: usize = 16;
const SUB_BITS: u32 = 3;
const SUB: usize = 1 << SUB_BITS;
const MAX_MSB: u32 = 47; // ~39 hours in ns: anything longer lands in the last bucket.
/// Number of buckets of a [`Histogram`].
pub const BUCKETS: usize = LINEAR + ((MAX_MSB - 4 + 1) as usize) * SUB;

/// A log-linear histogram of nanosecond durations: values below 16 ns are exact, above that
/// each power of two is split into 8 equal sub-buckets. Recording is a few integer operations.
#[derive(Clone, Debug)]
pub struct Histogram {
    buckets: Box<[u32]>,
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

impl Histogram {
    /// An empty histogram.
    pub fn new() -> Self {
        Self {
            buckets: vec![0; BUCKETS].into_boxed_slice(),
        }
    }

    fn index(ns: u64) -> usize {
        if ns < LINEAR as u64 {
            return ns as usize;
        }
        let msb = 63 - ns.leading_zeros();
        if msb > MAX_MSB {
            return BUCKETS - 1;
        }
        let sub = ((ns >> (msb - SUB_BITS)) as usize) & (SUB - 1);
        LINEAR + (msb as usize - 4) * SUB + sub
    }

    /// Inclusive upper bound (ns) of a bucket.
    fn upper(index: usize) -> u64 {
        if index < LINEAR {
            return index as u64;
        }
        let i = index - LINEAR;
        let msb = (i / SUB) as u32 + 4;
        let sub = (i % SUB) as u64;
        let width = 1u64 << (msb - SUB_BITS);
        ((SUB as u64 + sub) << (msb - SUB_BITS)) + width - 1
    }

    /// Record one value.
    pub fn record(&mut self, ns: u64) {
        let slot = &mut self.buckets[Self::index(ns)];
        *slot = slot.saturating_add(1);
    }

    /// Number of recorded values.
    pub fn count(&self) -> u64 {
        self.buckets.iter().map(|&c| u64::from(c)).sum()
    }

    /// The value (ns, bucket upper bound) under which `q` (0.0..=1.0) of the values fall.
    /// Returns 0 for an empty histogram.
    pub fn quantile(&self, q: f64) -> u64 {
        let total = self.count();
        if total == 0 {
            return 0;
        }
        let q = q.clamp(0.0, 1.0);
        let rank = ((q * total as f64).ceil() as u64).max(1);
        let mut seen = 0u64;
        for (i, &c) in self.buckets.iter().enumerate() {
            seen += u64::from(c);
            if seen >= rank {
                return Self::upper(i);
            }
        }
        Self::upper(BUCKETS - 1)
    }
}

/// Aggregated measurements of one section.
#[derive(Clone, Debug, Default)]
pub struct Stat {
    /// Number of calls.
    pub count: u64,
    /// Sum of durations (ns).
    pub total_ns: u64,
    /// Longest call (ns).
    pub max_ns: u64,
    /// Distribution.
    pub histogram: Histogram,
}

impl Stat {
    fn record(&mut self, ns: u64) {
        self.count += 1;
        self.total_ns = self.total_ns.saturating_add(ns);
        self.max_ns = self.max_ns.max(ns);
        self.histogram.record(ns);
    }
}

#[derive(Default)]
struct PluginData {
    sections: [HashMap<String, Stat>; SectionKind::ALL.len()],
}

struct Data {
    since: Instant,
    since_wall: SystemTime,
    plugins: HashMap<String, PluginData>,
    ticks: u64,
    tick_total_ns: u64,
    tick_max_ns: u64,
}

impl Data {
    fn new() -> Self {
        Self {
            since: Instant::now(),
            since_wall: SystemTime::now(),
            plugins: HashMap::new(),
            ticks: 0,
            tick_total_ns: 0,
            tick_max_ns: 0,
        }
    }
}

/// The timings collector. Cheap to share behind an `Arc`.
pub struct Timings {
    enabled: AtomicBool,
    data: Mutex<Data>,
}

impl std::fmt::Debug for Timings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timings")
            .field("enabled", &self.is_enabled())
            .finish_non_exhaustive()
    }
}

impl Default for Timings {
    fn default() -> Self {
        Self::new(false)
    }
}

fn duration_ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

impl Timings {
    /// A collector, initially on or off.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            data: Mutex::new(Data::new()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Data> {
        // A panic while holding the lock cannot leave the data inconsistent in a way that
        // matters for statistics: keep going.
        self.data.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether collection is on (one relaxed atomic load).
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Switch collection on or off. Switching on after being off resets the window start so
    /// that per-tick shares stay meaningful.
    pub fn set_enabled(&self, on: bool) {
        let was = self.enabled.swap(on, Ordering::Relaxed);
        if on && !was {
            let mut d = self.lock();
            if d.plugins.is_empty() && d.ticks == 0 {
                d.since = Instant::now();
                d.since_wall = SystemTime::now();
            }
        }
    }

    /// Forget everything collected so far.
    pub fn reset(&self) {
        *self.lock() = Data::new();
    }

    /// Start timing a section: `None` (and no clock read) when collection is off.
    #[inline]
    pub fn start(&self) -> Option<Instant> {
        if self.is_enabled() {
            Some(Instant::now())
        } else {
            None
        }
    }

    /// Finish a section started with [`Timings::start`]. No-op for `None`.
    #[inline]
    pub fn finish(&self, started: Option<Instant>, plugin: &str, kind: SectionKind, name: &str) {
        if let Some(t0) = started {
            self.record(plugin, kind, name, t0.elapsed());
        }
    }

    /// Record one measurement (ignored when collection is off).
    pub fn record(&self, plugin: &str, kind: SectionKind, name: &str, took: Duration) {
        if !self.is_enabled() {
            return;
        }
        let ns = duration_ns(took);
        let mut d = self.lock();
        let p = match d.plugins.get_mut(plugin) {
            Some(p) => p,
            None => d.plugins.entry(plugin.to_owned()).or_default(),
        };
        let map = &mut p.sections[kind.index()];
        match map.get_mut(name) {
            Some(stat) => stat.record(ns),
            None => map.entry(name.to_owned()).or_default().record(ns),
        }
    }

    /// Record the duration of one whole server tick (used for MSPT shares). Ignored when off.
    pub fn record_tick(&self, took: Duration) {
        if !self.is_enabled() {
            return;
        }
        let ns = duration_ns(took);
        let mut d = self.lock();
        d.ticks += 1;
        d.tick_total_ns = d.tick_total_ns.saturating_add(ns);
        d.tick_max_ns = d.tick_max_ns.max(ns);
    }

    /// Snapshot everything into a report (plugins sorted by total time, descending).
    pub fn report(&self) -> TimingsReport {
        let d = self.lock();
        let ticks = d.ticks;
        let tick_total_ns = d.tick_total_ns;
        let mut plugins: Vec<PluginReport> = d
            .plugins
            .iter()
            .map(|(id, p)| {
                let mut sections = Vec::new();
                for kind in SectionKind::ALL {
                    for (name, s) in &p.sections[kind.index()] {
                        sections.push(SectionReport::from_stat(kind, name, s));
                    }
                }
                sections.sort_by(|a, b| {
                    b.total_ms
                        .total_cmp(&a.total_ms)
                        .then_with(|| a.kind.cmp(&b.kind))
                        .then_with(|| a.name.cmp(&b.name))
                });
                let total_ns: u64 = p
                    .sections
                    .iter()
                    .flat_map(|m| m.values())
                    .map(|s| s.total_ns)
                    .fold(0u64, u64::saturating_add);
                let count = p
                    .sections
                    .iter()
                    .flat_map(|m| m.values())
                    .map(|s| s.count)
                    .sum();
                PluginReport {
                    plugin: id.clone(),
                    count,
                    total_ms: ns_to_ms(total_ns),
                    ms_per_tick: if ticks > 0 {
                        ns_to_ms(total_ns) / ticks as f64
                    } else {
                        0.0
                    },
                    mspt_share_percent: if tick_total_ns > 0 {
                        total_ns as f64 * 100.0 / tick_total_ns as f64
                    } else {
                        0.0
                    },
                    sections,
                }
            })
            .collect();
        plugins.sort_by(|a, b| {
            b.total_ms
                .total_cmp(&a.total_ms)
                .then_with(|| a.plugin.cmp(&b.plugin))
        });
        let started_unix_ms = d
            .since_wall
            .duration_since(UNIX_EPOCH)
            .map(|x| x.as_millis() as u64)
            .unwrap_or(0);
        TimingsReport {
            enabled: self.is_enabled(),
            started_unix_ms,
            window_ms: d.since.elapsed().as_secs_f64() * 1000.0,
            ticks,
            mean_mspt: if ticks > 0 {
                ns_to_ms(tick_total_ns) / ticks as f64
            } else {
                0.0
            },
            max_mspt: ns_to_ms(d.tick_max_ns),
            plugins,
        }
    }

    /// Write the current report as pretty JSON into `<dir>/<UTC timestamp>.json`
    /// (`20261109-153000.json`, with a numeric suffix if that file exists). Creates `dir`.
    pub fn export_json(&self, dir: &Path) -> std::io::Result<PathBuf> {
        let report = self.report();
        std::fs::create_dir_all(dir)?;
        let stamp = utc_timestamp(SystemTime::now());
        let mut path = dir.join(format!("{stamp}.json"));
        let mut n = 1;
        while path.exists() {
            path = dir.join(format!("{stamp}-{n}.json"));
            n += 1;
        }
        let json = serde_json::to_vec_pretty(&report).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&json)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &path)?;
        Ok(path)
    }
}

fn ns_to_ms(ns: u64) -> f64 {
    ns as f64 / 1_000_000.0
}

/// `YYYYMMDD-HHMMSS` in UTC.
pub fn utc_timestamp(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian (H. Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// One section in a report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SectionReport {
    /// What it is.
    pub kind: SectionKind,
    /// Event name, command name, task key or topic.
    pub name: String,
    /// Calls.
    pub count: u64,
    /// Total milliseconds.
    pub total_ms: f64,
    /// Mean milliseconds per call.
    pub mean_ms: f64,
    /// Longest call.
    pub max_ms: f64,
    /// Median (histogram bucket upper bound).
    pub p50_ms: f64,
    /// 95th percentile.
    pub p95_ms: f64,
    /// 99th percentile.
    pub p99_ms: f64,
}

impl SectionReport {
    fn from_stat(kind: SectionKind, name: &str, s: &Stat) -> Self {
        let clamp = |v: u64| ns_to_ms(v.min(s.max_ns));
        Self {
            kind,
            name: name.to_owned(),
            count: s.count,
            total_ms: ns_to_ms(s.total_ns),
            mean_ms: if s.count > 0 {
                ns_to_ms(s.total_ns) / s.count as f64
            } else {
                0.0
            },
            max_ms: ns_to_ms(s.max_ns),
            p50_ms: clamp(s.histogram.quantile(0.50)),
            p95_ms: clamp(s.histogram.quantile(0.95)),
            p99_ms: clamp(s.histogram.quantile(0.99)),
        }
    }
}

/// One plugin in a report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginReport {
    /// Plugin id.
    pub plugin: String,
    /// Calls into the plugin.
    pub count: u64,
    /// Total milliseconds spent in the plugin.
    pub total_ms: f64,
    /// Average milliseconds per recorded tick.
    pub ms_per_tick: f64,
    /// Share of the measured tick time, in percent (0 when no tick was recorded).
    pub mspt_share_percent: f64,
    /// Sections, longest total first.
    pub sections: Vec<SectionReport>,
}

/// A full timings report (`/rustic timings report`, JSON export).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimingsReport {
    /// Whether collection was on when the report was taken.
    pub enabled: bool,
    /// Start of the collection window (Unix ms).
    pub started_unix_ms: u64,
    /// Length of the collection window.
    pub window_ms: f64,
    /// Ticks recorded with [`Timings::record_tick`].
    pub ticks: u64,
    /// Mean measured tick duration.
    pub mean_mspt: f64,
    /// Longest measured tick.
    pub max_mspt: f64,
    /// Plugins, most expensive first.
    pub plugins: Vec<PluginReport>,
}

impl TimingsReport {
    /// A fixed-width text table, at most `max_sections` lines per plugin.
    pub fn to_text(&self, max_sections: usize) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "Timings ({}): window {:.1} s, {} ticks, mean {:.2} mspt, max {:.2} ms",
            if self.enabled { "on" } else { "off" },
            self.window_ms / 1000.0,
            self.ticks,
            self.mean_mspt,
            self.max_mspt
        );
        if self.plugins.is_empty() {
            out.push_str("No data. Turn timings on with /rustic timings on.\n");
            return out;
        }
        let _ = writeln!(
            out,
            "{:<34} {:>8} {:>10} {:>9} {:>9} {:>9} {:>9} {:>9}",
            "plugin / section", "count", "total ms", "mean ms", "p50", "p95", "p99", "max"
        );
        for p in &self.plugins {
            let _ = writeln!(
                out,
                "{:<34} {:>8} {:>10.2} {:>9} {:>9} {:>9} {:>9} {:>9}  {:.3} ms/tick, {:.1}% of tick",
                truncate(&p.plugin, 34),
                p.count,
                p.total_ms,
                "",
                "",
                "",
                "",
                "",
                p.ms_per_tick,
                p.mspt_share_percent
            );
            for s in p.sections.iter().take(max_sections) {
                let label = format!("  {} {}", s.kind.label(), s.name);
                let _ = writeln!(
                    out,
                    "{:<34} {:>8} {:>10.2} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>9.3}",
                    truncate(&label, 34),
                    s.count,
                    s.total_ms,
                    s.mean_ms,
                    s.p50_ms,
                    s.p95_ms,
                    s.p99_ms,
                    s.max_ms
                );
            }
            if p.sections.len() > max_sections {
                let _ = writeln!(out, "  ... {} more", p.sections.len() - max_sections);
            }
        }
        out
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('~');
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_buckets_are_monotonic_and_cover_values() {
        let mut prev = 0;
        for i in 0..BUCKETS {
            let up = Histogram::upper(i);
            assert!(i == 0 || up > prev, "bucket {i}");
            prev = up;
        }
        for v in [
            0u64,
            1,
            15,
            16,
            17,
            100,
            1_000,
            123_456,
            50_000_000,
            1 << 40,
        ] {
            let i = Histogram::index(v);
            assert!(Histogram::upper(i) >= v, "{v}");
            if i > 0 {
                assert!(Histogram::upper(i - 1) < v, "{v}");
            }
        }
    }

    #[test]
    fn quantiles_are_close() {
        let mut h = Histogram::new();
        for v in 1..=1000u64 {
            h.record(v * 1000);
        }
        let p50 = h.quantile(0.5) as f64;
        let p99 = h.quantile(0.99) as f64;
        assert!((p50 - 500_000.0).abs() / 500_000.0 < 0.07, "{p50}");
        assert!((p99 - 990_000.0).abs() / 990_000.0 < 0.07, "{p99}");
        assert_eq!(Histogram::new().quantile(0.5), 0);
    }

    #[test]
    fn off_records_nothing() {
        let t = Timings::new(false);
        assert!(t.start().is_none());
        t.record(
            "a",
            SectionKind::Event,
            "player-join",
            Duration::from_millis(1),
        );
        t.record_tick(Duration::from_millis(50));
        let r = t.report();
        assert!(r.plugins.is_empty());
        assert_eq!(r.ticks, 0);
    }

    #[test]
    fn report_aggregates_and_sorts() {
        let t = Timings::new(true);
        for _ in 0..10 {
            t.record(
                "a",
                SectionKind::Event,
                "player-join",
                Duration::from_micros(100),
            );
        }
        t.record("a", SectionKind::Command, "home", Duration::from_millis(3));
        t.record(
            "b",
            SectionKind::Task,
            "run-later",
            Duration::from_millis(10),
        );
        for _ in 0..4 {
            t.record_tick(Duration::from_millis(50));
        }
        let r = t.report();
        assert_eq!(r.ticks, 4);
        assert!((r.mean_mspt - 50.0).abs() < 1e-9);
        assert_eq!(r.plugins[0].plugin, "b");
        assert_eq!(r.plugins[1].plugin, "a");
        let a = &r.plugins[1];
        assert_eq!(a.count, 11);
        assert!((a.total_ms - 4.0).abs() < 1e-9);
        assert_eq!(a.sections[0].kind, SectionKind::Command);
        assert!((a.ms_per_tick - 1.0).abs() < 1e-9);
        assert!((a.mspt_share_percent - 2.0).abs() < 1e-9);
        let join = &a.sections[1];
        assert_eq!(join.count, 10);
        assert!(join.p50_ms <= 0.1 + 1e-9 && join.p50_ms > 0.09);
        let text = r.to_text(10);
        assert!(text.contains("player-join"));
        assert!(text.contains("% of tick"));
        t.reset();
        assert!(t.report().plugins.is_empty());
    }

    #[test]
    fn toggling_and_finish() {
        let t = Timings::new(false);
        t.set_enabled(true);
        let s = t.start();
        assert!(s.is_some());
        t.finish(s, "x", SectionKind::Lifecycle, "on-enable");
        t.set_enabled(false);
        t.finish(t.start(), "x", SectionKind::Lifecycle, "on-disable");
        let r = t.report();
        assert_eq!(r.plugins[0].sections.len(), 1);
        assert!(!r.enabled);
    }

    #[test]
    fn json_export_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let t = Timings::new(true);
        t.record(
            "a",
            SectionKind::Event,
            "chunk-load",
            Duration::from_micros(5),
        );
        let p1 = t.export_json(&dir.path().join("timings")).unwrap();
        let p2 = t.export_json(&dir.path().join("timings")).unwrap();
        assert_ne!(p1, p2);
        let back: TimingsReport = serde_json::from_slice(&std::fs::read(&p1).unwrap()).unwrap();
        assert_eq!(back.plugins[0].plugin, "a");
    }

    #[test]
    fn timestamps() {
        assert_eq!(utc_timestamp(UNIX_EPOCH), "19700101-000000");
        let t = UNIX_EPOCH + Duration::from_secs(1_762_700_000); // 2025-11-09T14:53:20Z
        assert_eq!(utc_timestamp(t), "20251109-145320");
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }
}
