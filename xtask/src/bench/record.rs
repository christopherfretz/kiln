//! The on-disk records: `bench/results/<timestamp>.json` (one run) and
//! `bench/baselines/<group>.json` (committed references). Schemas are in
//! `spec/bench.md` "Records".

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::entry::Mode;
use crate::util::header::Counter;
use crate::util::json::Json;
use crate::util::oven_report::Counters;

/// Version of both record formats. Bump on an incompatible change.
pub const SCHEMA: u64 = 1;

/// The machine a measurement was taken on. Times are only comparable
/// between identical hosts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Host {
    pub arch: String,
    pub cpus: usize,
}

impl Host {
    pub fn current() -> Self {
        Self {
            arch: std::env::consts::ARCH.to_owned(),
            cpus: std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
        }
    }

    fn to_json(&self) -> Json {
        #[allow(clippy::cast_precision_loss)]
        Json::obj([("arch", Json::str(&self.arch)), ("cpus", Json::Num(self.cpus as f64))])
    }

    fn from_json(j: &Json) -> Result<Self, String> {
        Ok(Self {
            arch: field(j, "arch")?.as_str().ok_or("`arch` must be a string")?.to_owned(),
            cpus: usize::try_from(
                field(j, "cpus")?.as_u64().ok_or("`cpus` must be a non-negative integer")?,
            )
            .map_err(|_| "`cpus` out of range")?,
        })
    }
}

/// One entry measured with one compiler.
#[derive(Clone, Debug, PartialEq)]
pub struct Measurement {
    pub mode: Mode,
    /// The compiler arguments, minus the file name, as one string.
    pub flags: String,
    /// `entry::source_hash` of the entry when it was measured.
    pub source_hash: String,
    pub runs: usize,
    pub wall_ms_median: f64,
    pub wall_ms_min: f64,
    pub wall_ms_max: f64,
    pub spread_pct: f64,
    /// Median over the runs of the child's peak RSS; `None` without a probe.
    pub max_rss_kb: Option<u64>,
    /// The total line of kiln's constexpr report (identical in every run;
    /// `wall_us` is not kept). `None` for other compilers.
    pub oven: Option<Counters>,
    /// Counters a baseline recorded before they existed lacks
    /// ([`OPTIONAL_COUNTERS`]): read as 0, not judged, not written back.
    /// Empty for a fresh measurement.
    pub oven_missing: Vec<Counter>,
    /// Per-run samples, in run order. Results keep them; baselines do not.
    pub samples_wall_ms: Vec<f64>,
    pub samples_rss_kb: Vec<u64>,
    /// The compiler as given on the command line.
    pub compiler: String,
    /// The first line of `<compiler> --version`.
    pub compiler_version: String,
    pub host: Host,
}

#[allow(clippy::cast_precision_loss)]
fn num_u(n: u64) -> Json {
    Json::Num(n as f64)
}

impl Measurement {
    fn to_json(&self, samples: bool, recorded: Option<&str>) -> Json {
        let mut p = vec![
            ("mode", Json::str(self.mode.as_str())),
            ("flags", Json::str(&self.flags)),
            ("source_hash", Json::str(&self.source_hash)),
            ("runs", num_u(self.runs as u64)),
            ("wall_ms_median", Json::Num(self.wall_ms_median)),
            ("wall_ms_min", Json::Num(self.wall_ms_min)),
            ("wall_ms_max", Json::Num(self.wall_ms_max)),
            ("spread_pct", Json::Num(self.spread_pct)),
            ("max_rss_kb", self.max_rss_kb.map_or(Json::Null, num_u)),
        ];
        if let Some(c) = &self.oven {
            let keys = Counter::ALL.into_iter().filter(|k| !self.oven_missing.contains(k));
            p.push(("oven", Json::obj(keys.map(|k| (k.name(), num_u(c.get(k)))))));
        }
        if samples {
            p.push((
                "samples_wall_ms",
                Json::Arr(self.samples_wall_ms.iter().map(|&x| Json::Num(x)).collect()),
            ));
            p.push((
                "samples_rss_kb",
                Json::Arr(self.samples_rss_kb.iter().map(|&x| num_u(x)).collect()),
            ));
        }
        p.push(("compiler", Json::str(&self.compiler)));
        p.push(("compiler_version", Json::str(&self.compiler_version)));
        p.push(("host", self.host.to_json()));
        if let Some(r) = recorded {
            p.push(("recorded", Json::str(r)));
        }
        Json::obj(p)
    }

    fn from_json(j: &Json) -> Result<Self, String> {
        let s = |k: &str| -> Result<String, String> {
            Ok(field(j, k)?.as_str().ok_or_else(|| format!("`{k}` must be a string"))?.to_owned())
        };
        let f = |k: &str| -> Result<f64, String> {
            field(j, k)?.as_f64().ok_or_else(|| format!("`{k}` must be a number"))
        };
        let mode = s("mode")?;
        let rss = field(j, "max_rss_kb")?;
        let (oven, oven_missing) = match j.get("oven") {
            Some(o) => oven_from_json(o).map(|(c, m)| (Some(c), m))?,
            None => (None, Vec::new()),
        };
        let nums = |k: &str| -> Result<Vec<&Json>, String> {
            Ok(match j.get(k) {
                None => Vec::new(),
                Some(a) => {
                    a.as_arr().ok_or_else(|| format!("`{k}` must be an array"))?.iter().collect()
                }
            })
        };
        Ok(Self {
            mode: Mode::parse(&mode).ok_or_else(|| format!("unknown mode `{mode}`"))?,
            flags: s("flags")?,
            source_hash: s("source_hash")?,
            runs: usize::try_from(field(j, "runs")?.as_u64().ok_or("`runs` must be an integer")?)
                .map_err(|_| "`runs` out of range")?,
            wall_ms_median: f("wall_ms_median")?,
            wall_ms_min: f("wall_ms_min")?,
            wall_ms_max: f("wall_ms_max")?,
            spread_pct: f("spread_pct")?,
            max_rss_kb: match rss {
                Json::Null => None,
                r => Some(r.as_u64().ok_or("`max_rss_kb` must be an integer or null")?),
            },
            oven,
            oven_missing,
            samples_wall_ms: nums("samples_wall_ms")?
                .into_iter()
                .map(|x| x.as_f64().ok_or("`samples_wall_ms` must hold numbers"))
                .collect::<Result<_, _>>()?,
            samples_rss_kb: nums("samples_rss_kb")?
                .into_iter()
                .map(|x| x.as_u64().ok_or("`samples_rss_kb` must hold integers"))
                .collect::<Result<_, _>>()?,
            compiler: s("compiler")?,
            compiler_version: s("compiler_version")?,
            host: Host::from_json(field(j, "host")?)?,
        })
    }
}

/// Counters added after baselines were first recorded: a baseline without
/// them still loads and is compared on the counters it has.
pub const OPTIONAL_COUNTERS: [Counter; 2] = [Counter::PeakArena, Counter::FrozenBytes];

/// The counters, and which optional ones are absent.
fn oven_from_json(j: &Json) -> Result<(Counters, Vec<Counter>), String> {
    let mut missing = Vec::new();
    let mut n = |c: Counter| -> Result<u64, String> {
        match j.get(c.name()) {
            None if OPTIONAL_COUNTERS.contains(&c) => {
                missing.push(c);
                Ok(0)
            }
            None => Err(format!("missing `{}`", c.name())),
            Some(v) => v
                .as_u64()
                .ok_or_else(|| format!("`oven.{}` must be a non-negative integer", c.name())),
        }
    };
    let c = Counters {
        steps: n(Counter::Steps)?,
        bytes: n(Counter::Bytes)?,
        peak_bytes: n(Counter::PeakBytes)?,
        calls: n(Counter::Calls)?,
        allocs: n(Counter::Allocs)?,
        peak_arena: n(Counter::PeakArena)?,
        frozen_bytes: n(Counter::FrozenBytes)?,
        wall_us: 0,
        evaluations: n(Counter::Evaluations)?,
    };
    Ok((c, missing))
}

fn field<'j>(j: &'j Json, key: &str) -> Result<&'j Json, String> {
    j.get(key).ok_or_else(|| format!("missing `{key}`"))
}

fn check_schema(j: &Json) -> Result<(), String> {
    match j.get("schema").and_then(Json::as_u64) {
        Some(SCHEMA) => Ok(()),
        Some(n) => Err(format!("unsupported schema {n} (this harness reads {SCHEMA})")),
        None => Err("missing `schema`".into()),
    }
}

/// The outcome of one entry with one compiler.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant, reason = "a handful per run")]
pub enum Outcome {
    Measured(Measurement),
    /// The entry could not be measured (directive error, compile error,
    /// crash, timeout). The string says why.
    Failed(String),
}

impl Outcome {
    pub fn measurement(&self) -> Option<&Measurement> {
        match self {
            Self::Measured(m) => Some(m),
            Self::Failed(_) => None,
        }
    }

    fn to_json(&self) -> Json {
        match self {
            Self::Measured(m) => {
                let Json::Obj(mut p) = m.to_json(true, None) else { unreachable!("an object") };
                p.insert(0, ("status".into(), Json::str("ok")));
                Json::Obj(p)
            }
            Self::Failed(why) => {
                Json::obj([("status", Json::str("failed")), ("error", Json::str(why))])
            }
        }
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read back by tests and future trend tooling")
    )]
    fn from_json(j: &Json) -> Result<Self, String> {
        match field(j, "status")?.as_str() {
            Some("ok") => Measurement::from_json(j).map(Self::Measured),
            Some("failed") => Ok(Self::Failed(
                field(j, "error")?.as_str().ok_or("`error` must be a string")?.to_owned(),
            )),
            _ => Err("`status` must be \"ok\" or \"failed\"".into()),
        }
    }
}

/// One compiler's side of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct CompilerRun {
    pub compiler: String,
    pub kind: String,
    pub version: String,
    /// Keyed by entry id (`<group>/<name>`).
    pub entries: BTreeMap<String, Outcome>,
}

impl CompilerRun {
    fn to_json(&self) -> Json {
        Json::obj([
            ("compiler", Json::str(&self.compiler)),
            ("compiler_kind", Json::str(&self.kind)),
            ("compiler_version", Json::str(&self.version)),
            ("entries", Json::obj(self.entries.iter().map(|(k, v)| (k.clone(), v.to_json())))),
        ])
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read back by tests and future trend tooling")
    )]
    fn from_json(j: &Json) -> Result<Self, String> {
        let s = |k: &str| -> Result<String, String> {
            Ok(field(j, k)?.as_str().ok_or_else(|| format!("`{k}` must be a string"))?.to_owned())
        };
        let entries = field(j, "entries")?
            .as_obj()
            .ok_or("`entries` must be an object")?
            .iter()
            .map(|(k, v)| {
                Outcome::from_json(v).map(|o| (k.clone(), o)).map_err(|e| format!("{k}: {e}"))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            compiler: s("compiler")?,
            kind: s("compiler_kind")?,
            version: s("compiler_version")?,
            entries,
        })
    }
}

/// A `bench/results/<timestamp>.json` file.
#[derive(Clone, Debug, PartialEq)]
pub struct RunRecord {
    pub timestamp: String,
    pub host: Host,
    pub load_avg_1m: Option<f64>,
    /// `RssProbe::source_id`.
    pub rss_source: String,
    pub runs: usize,
    pub test: CompilerRun,
    pub oracle: Option<CompilerRun>,
}

impl RunRecord {
    pub fn to_json(&self) -> Json {
        Json::obj([
            ("schema", num_u(SCHEMA)),
            ("timestamp", Json::str(&self.timestamp)),
            ("host", self.host.to_json()),
            ("load_avg_1m", Json::opt_num(self.load_avg_1m)),
            ("rss_source", Json::str(&self.rss_source)),
            ("runs", num_u(self.runs as u64)),
            ("test", self.test.to_json()),
            ("oracle", self.oracle.as_ref().map_or(Json::Null, CompilerRun::to_json)),
        ])
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read back by tests and future trend tooling")
    )]
    pub fn from_json(j: &Json) -> Result<Self, String> {
        check_schema(j)?;
        Ok(Self {
            timestamp: field(j, "timestamp")?
                .as_str()
                .ok_or("`timestamp` must be a string")?
                .into(),
            host: Host::from_json(field(j, "host")?)?,
            load_avg_1m: field(j, "load_avg_1m")?.as_f64(),
            rss_source: field(j, "rss_source")?
                .as_str()
                .ok_or("`rss_source` must be a string")?
                .into(),
            runs: usize::try_from(field(j, "runs")?.as_u64().ok_or("`runs` must be an integer")?)
                .map_err(|_| "`runs` out of range")?,
            test: CompilerRun::from_json(field(j, "test")?)?,
            oracle: match field(j, "oracle")? {
                Json::Null => None,
                o => Some(CompilerRun::from_json(o)?),
            },
        })
    }
}

/// A baseline: the reference measurement of one entry for one compiler kind.
#[derive(Clone, Debug, PartialEq)]
pub struct Baseline {
    pub measurement: Measurement,
    /// Timestamp of the run that recorded it.
    pub recorded: String,
}

/// `bench/baselines/<group>.json`: entry name -> compiler kind -> baseline.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BaselineFile {
    pub entries: BTreeMap<String, BTreeMap<String, Baseline>>,
}

impl BaselineFile {
    pub fn to_json(&self, group: &str) -> Json {
        let entries = self.entries.iter().map(|(name, kinds)| {
            let kinds = kinds
                .iter()
                .map(|(kind, b)| (kind.clone(), b.measurement.to_json(false, Some(&b.recorded))));
            (name.clone(), Json::obj(kinds))
        });
        Json::obj([
            ("schema", num_u(SCHEMA)),
            ("group", Json::str(group)),
            ("entries", Json::obj(entries)),
        ])
    }

    pub fn from_json(j: &Json, group: &str) -> Result<Self, String> {
        check_schema(j)?;
        match field(j, "group")?.as_str() {
            Some(g) if g == group => {}
            g => return Err(format!("`group` is {g:?}, expected {group:?}")),
        }
        let mut out = Self::default();
        for (name, kinds) in field(j, "entries")?.as_obj().ok_or("`entries` must be an object")? {
            let mut map = BTreeMap::new();
            for (kind, b) in kinds.as_obj().ok_or_else(|| format!("{name}: must be an object"))? {
                let measurement =
                    Measurement::from_json(b).map_err(|e| format!("{name}.{kind}: {e}"))?;
                let recorded = field(b, "recorded")
                    .and_then(|r| r.as_str().ok_or_else(|| "`recorded` must be a string".into()))
                    .map_err(|e| format!("{name}.{kind}: {e}"))?
                    .to_owned();
                map.insert(kind.clone(), Baseline { measurement, recorded });
            }
            out.entries.insert(name.clone(), map);
        }
        Ok(out)
    }

    /// Reads `path`; a missing file is an empty baseline.
    pub fn load(path: &Path, group: &str) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Json::parse(&text)
                .and_then(|j| Self::from_json(&j, group))
                .map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
}

/// `YYYY-MM-DDTHHMMSS` in UTC (the results file name).
#[allow(clippy::many_single_char_names)]
pub fn timestamp(t: SystemTime) -> String {
    let (y, mo, d, h, mi, s) = civil(t);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}{mi:02}{s:02}")
}

/// `YYYY-MM-DD HH:MM:SS UTC`, for humans.
#[allow(clippy::many_single_char_names)]
pub fn human_time(t: SystemTime) -> String {
    let (y, mo, d, h, mi, s) = civil(t);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02} UTC")
}

/// UTC calendar fields of `t` (Howard Hinnant's `civil_from_days`).
#[allow(clippy::many_single_char_names)]
fn civil(t: SystemTime) -> (i64, u32, u32, u32, u32, u32) {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, (rem / 3600) as u32, (rem % 3600 / 60) as u32, (rem % 60) as u32)
}

#[cfg(test)]
pub(super) mod tests {
    use std::time::Duration;

    use super::*;

    pub fn sample(compiler: &str, median: f64, rss: Option<u64>) -> Measurement {
        Measurement {
            mode: Mode::Syntax,
            flags: "-std=c++26 -fsyntax-only".into(),
            source_hash: "fnv1a64:0000000000000001".into(),
            runs: 3,
            wall_ms_median: median,
            wall_ms_min: median * 0.98,
            wall_ms_max: median * 1.1,
            spread_pct: 2.04,
            max_rss_kb: rss,
            oven: None,
            oven_missing: Vec::new(),
            samples_wall_ms: vec![median * 1.1, median, median * 0.98],
            samples_rss_kb: rss.into_iter().collect(),
            compiler: compiler.into(),
            compiler_version: "Ubuntu clang version 19.1.7".into(),
            host: Host { arch: "aarch64".into(), cpus: 24 },
        }
    }

    #[test]
    fn timestamps() {
        let t = UNIX_EPOCH + Duration::from_secs(1_793_190_896); // 2026-10-28 12:34:56
        assert_eq!(timestamp(t), "2026-10-28T123456");
        assert_eq!(human_time(t), "2026-10-28 12:34:56 UTC");
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01T000000");
        // A leap day.
        let leap = Duration::from_hours(264_384); // 951_782_400 s
        assert_eq!(timestamp(UNIX_EPOCH + leap), "2000-02-29T000000");
    }

    #[test]
    fn run_record_round_trips() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "oven/fib".to_owned(),
            Outcome::Measured(sample("clang++-19", 712.345, Some(86_132))),
        );
        entries.insert("pp/vector".to_owned(), Outcome::Failed("exit code 1: error: x".into()));
        entries.insert("pp/zz".to_owned(), Outcome::Measured(sample("clang++-19", 70.0, None)));
        let mut kiln = sample("kiln", 9.0, Some(1000));
        kiln.oven = Some(Counters {
            steps: 1,
            bytes: 2,
            peak_bytes: 3,
            calls: 4,
            allocs: 5,
            peak_arena: 7,
            frozen_bytes: 8,
            wall_us: 0,
            evaluations: 6,
        });
        entries.insert("oven/kiln".to_owned(), Outcome::Measured(kiln));
        let r = RunRecord {
            timestamp: "2026-10-28T123456".into(),
            host: Host { arch: "aarch64".into(), cpus: 24 },
            load_avg_1m: Some(3.5),
            rss_source: "gnu-time".into(),
            runs: 3,
            test: CompilerRun {
                compiler: "clang++-19".into(),
                kind: "clang".into(),
                version: "Ubuntu clang version 19.1.7".into(),
                entries,
            },
            oracle: None,
        };
        let text = r.to_json().to_pretty();
        assert_eq!(RunRecord::from_json(&Json::parse(&text).unwrap()).unwrap(), r);
        // Stable key order: status first, samples before compiler identity.
        let fib = text.find("\"oven/fib\"").unwrap();
        let tail = &text[fib..];
        let order = [
            "\"status\"",
            "\"mode\"",
            "\"wall_ms_median\"",
            "\"max_rss_kb\"",
            "\"samples_wall_ms\"",
            "\"compiler\"",
            "\"host\"",
        ];
        let pos: Vec<usize> = order.iter().map(|k| tail.find(k).unwrap()).collect();
        assert!(pos.windows(2).all(|w| w[0] < w[1]), "{text}");
        assert!(text.contains("\"max_rss_kb\": null"));
        // Counters follow max_rss_kb, and only where there are some.
        let k = &text[text.find("\"oven/kiln\"").unwrap()..];
        let k = &k[..k.find("\"compiler\"").unwrap()];
        assert!(
            k.contains(
                "\"oven\": {\n          \"steps\": 1,\n          \"bytes\": 2,\n          \
                 \"peak_bytes\": 3,\n          \"calls\": 4,\n          \"allocs\": 5,\n          \
                 \"peak_arena\": 7,\n          \"frozen_bytes\": 8,\n          \
                 \"evaluations\": 6\n        }"
            ),
            "{k}"
        );
        assert_eq!(text.matches("\"oven\":").count(), 1, "{text}");
    }

    #[test]
    fn baseline_file_round_trips_without_samples() {
        let mut f = BaselineFile::default();
        let b = Baseline {
            measurement: sample("clang++-19", 500.0, Some(90_000)),
            recorded: "2026-10-28T000000".into(),
        };
        f.entries.entry("fib".into()).or_default().insert("clang".into(), b.clone());
        let text = f.to_json("oven").to_pretty();
        assert!(!text.contains("samples"), "{text}");
        let back = BaselineFile::from_json(&Json::parse(&text).unwrap(), "oven").unwrap();
        let got = &back.entries["fib"]["clang"];
        assert_eq!(got.recorded, b.recorded);
        assert_eq!(got.measurement.wall_ms_median, 500.0);
        assert_eq!(got.measurement.samples_wall_ms, Vec::<f64>::new());
        assert!(
            BaselineFile::from_json(&Json::parse(&text).unwrap(), "pp")
                .unwrap_err()
                .contains("group")
        );
        let future = text.replace("\"schema\": 1", "\"schema\": 2");
        assert!(
            BaselineFile::from_json(&Json::parse(&future).unwrap(), "oven")
                .unwrap_err()
                .contains("schema")
        );
        assert_eq!(
            BaselineFile::load(Path::new("/nonexistent/x.json"), "oven").unwrap(),
            BaselineFile::default()
        );
    }

    #[test]
    fn baselines_without_the_newer_counters_still_load() {
        let mut meas = sample("kiln", 9.0, Some(1000));
        meas.oven =
            Some(Counters { steps: 1, peak_arena: 7, frozen_bytes: 8, ..Counters::default() });
        let mut file = BaselineFile::default();
        let base = Baseline { measurement: meas, recorded: "2026-10-28T000000".into() };
        file.entries.entry("e".into()).or_default().insert("kiln".into(), base);
        let text = file.to_json("oven").to_pretty();
        let back = BaselineFile::from_json(&Json::parse(&text).unwrap(), "oven").unwrap();
        assert_eq!(back.entries["e"]["kiln"].measurement.oven_missing, []);
        // An older baseline has neither key: read as 0, marked missing, and
        // written back without them.
        let old = text.replace("\"peak_arena\": 7,", "").replace("\"frozen_bytes\": 8,", "");
        assert!(!old.contains("peak_arena") && !old.contains("frozen_bytes"), "{old}");
        let back = BaselineFile::from_json(&Json::parse(&old).unwrap(), "oven").unwrap();
        let got = &back.entries["e"]["kiln"].measurement;
        assert_eq!(got.oven_missing, OPTIONAL_COUNTERS);
        let counters = got.oven.unwrap();
        assert_eq!((counters.steps, counters.peak_arena, counters.frozen_bytes), (1, 0, 0));
        let again = back.to_json("oven").to_pretty();
        assert!(!again.contains("peak_arena"), "{again}");
        // The counters that existed from the start stay required.
        let bad = text.replace("\"allocs\": 0,", "");
        let err = BaselineFile::from_json(&Json::parse(&bad).unwrap(), "oven").unwrap_err();
        assert!(err.contains("missing `allocs`"), "{err}");
    }
}
