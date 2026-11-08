//! `cargo xtask bench --scaling <shape>`: one reflection shape of
//! `cargo xtask gen-reflect` at N and 4N (`spec/bench.md` "Scaling sweeps";
//! the gate of `docs/plans/m6-reflection.md` §3.8).
//!
//! Each size is generated into run scratch (never into the repo), compiled
//! `--runs` times at `-fsyntax-only`, and reported with its fastest wall
//! time, its largest peak RSS and, for kiln, the `-fconstexpr-report`
//! totals. A 4N/N ratio of wall time or peak RSS above [`MAX_RATIO`] fails
//! (quadratic behaviour, whatever the host's speed); for the shapes whose
//! evaluations do not grow with N ([`FLAT_ARENA`]) a larger `peak_arena` at
//! 4N fails too. Off the merge queue: the dev host runs it by hand or
//! nightly, with `--compiler` naming GCC 16.1 for the vs-GCC table.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::Options;
use crate::gen_reflect::{self, Shape};
use crate::util::oven_report::{self, Counters};
use crate::util::process::{self, MemLimit, RssProbe};
use crate::util::{CompilerId, CompilerKind};

/// The §3.8 scaling gate: 4x the input may cost at most this much more.
pub const MAX_RATIO: f64 = 4.6;

/// Shapes whose every evaluation is independent of N: `peak_arena` must
/// not grow from N to 4N.
pub const FLAT_ARENA: &[&str] = &["s1", "s2", "s4", "s5", "s8"];

/// A sweep compile may take this long.
const TIMEOUT: Duration = Duration::from_secs(600);

/// One size of the sweep.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Point {
    pub n: usize,
    /// The fastest run, in milliseconds.
    pub wall_ms: f64,
    /// The largest peak RSS of the runs, in KiB (`None`: no probe).
    pub rss_kb: Option<u64>,
    /// kiln's report totals.
    pub counters: Option<Counters>,
    /// Why the compile failed, if it did (the first stderr lines).
    pub failed: Option<String>,
}

/// The verdicts on a pair of points.
pub fn judge(shape: &str, small: &Point, large: &Point) -> Vec<String> {
    let mut out = Vec::new();
    let mut ratio = |what: &str, a: f64, b: f64| {
        if a > 0.0 && b / a > MAX_RATIO {
            out.push(format!("{what} grows {:.2}x from N to 4N (limit {MAX_RATIO}x)", b / a));
        }
    };
    ratio("wall time", small.wall_ms, large.wall_ms);
    #[allow(clippy::cast_precision_loss)]
    if let (Some(a), Some(b)) = (small.rss_kb, large.rss_kb) {
        ratio("peak RSS", a as f64, b as f64);
    }
    if let (Some(a), Some(b)) = (small.counters, large.counters)
        && FLAT_ARENA.contains(&shape)
        && b.peak_arena > a.peak_arena
    {
        out.push(format!(
            "peak_arena grows from {} to {} (must not depend on N)",
            a.peak_arena, b.peak_arena
        ));
    }
    out
}

/// `cargo xtask bench --scaling <shape>`. `Ok(false)` is a failed gate or
/// compile (exit status 1).
pub fn run(
    opts: &Options,
    scratch: &Path,
    compiler: &Path,
    id: &CompilerId,
) -> Result<bool, String> {
    let name = opts.scaling.as_deref().unwrap_or_default();
    let shape = gen_reflect::shape(name)
        .ok_or_else(|| format!("unknown --scaling shape `{name}` (s1..s10)"))?;
    let n = opts.scaling_n.unwrap_or(shape.default_n);
    if !n.is_multiple_of(shape.step) {
        return Err(format!("shape {} needs --n to be a multiple of {}", shape.id, shape.step));
    }
    std::fs::create_dir_all(scratch)
        .map_err(|e| format!("cannot create {}: {e}", scratch.display()))?;
    let probe = RssProbe::detect(scratch);
    println!(
        "scaling: {} at N={n} and 4N={} with {} ({}), {} runs; {}",
        shape.id.to_uppercase(),
        4 * n,
        compiler.display(),
        id.version,
        opts.runs,
        probe.describe()
    );
    let cap = MemLimit::from_mib(opts.mem_limit_mib);
    let mut points = Vec::new();
    for size in [n, 4 * n] {
        let p = measure(shape, size, scratch, compiler, id.kind, &probe, opts.runs, cap)?;
        eprintln!("  N={size}: {}", p.failed.as_deref().map_or("measured", |_| "failed"));
        points.push(p);
    }
    print!("{}", render(&points));
    let xfail = shape_xfail(shape).filter(|_| id.kind == CompilerKind::Kiln);
    let failed: Vec<&str> = points.iter().filter_map(|p| p.failed.as_deref()).collect();
    if !failed.is_empty() {
        if let Some(reason) = xfail {
            println!("scaling result: xfail (`xfail(kiln): {reason}`)\n{}", failed.join("\n"));
            return Ok(true);
        }
        println!("scaling result: FAILED (the compile failed)\n{}", failed.join("\n"));
        return Ok(false);
    }
    let verdicts = judge(shape.id, &points[0], &points[1]);
    if let Some(reason) = xfail {
        println!("scaling result: XPASS: compiled although `xfail(kiln): {reason}`");
        return Ok(false);
    }
    if verdicts.is_empty() {
        println!("scaling result: ok");
        Ok(true)
    } else {
        println!("scaling result: FAILED\n  {}", verdicts.join("\n  "));
        Ok(false)
    }
}

/// The `xfail(kiln)` reason the shape's entries carry.
fn shape_xfail(shape: &Shape) -> Option<String> {
    super::entry::parse_spec(&gen_reflect::generate(shape, shape.default_n))
        .ok()
        .and_then(|s| s.xfail_kiln)
}

#[allow(clippy::too_many_arguments)]
fn measure(
    shape: &Shape,
    n: usize,
    scratch: &Path,
    compiler: &Path,
    kind: CompilerKind,
    probe: &RssProbe,
    runs: usize,
    cap: Option<MemLimit>,
) -> Result<Point, String> {
    let file = scratch.join(format!("{}-{n}.cpp", shape.id));
    std::fs::write(&file, gen_reflect::generate(shape, n))
        .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
    let spec = super::entry::parse_spec(&gen_reflect::generate(shape, n))
        .map_err(|e| format!("generated {} has bad directives: {e:?}", shape.id))?;
    let mut cmd = Command::new(compiler);
    cmd.arg(format!("-std={}", spec.std)).arg("-fsyntax-only").args(&spec.args);
    if kind == CompilerKind::Kiln {
        cmd.args(gen_reflect::kiln_flags(shape));
    }
    let report =
        (kind == CompilerKind::Kiln).then(|| scratch.join(format!("{}-{n}.jsonl", shape.id)));
    if let Some(r) = &report {
        let mut flag = std::ffi::OsString::from(oven_report::FLAG);
        flag.push(r);
        cmd.arg(flag);
    }
    cmd.arg(&file).current_dir(scratch);
    let cap = cap.map(|c| {
        if kind == CompilerKind::Kiln {
            c.plus(process::kiln_stack_reservation_mib(&spec.args))
        } else {
            c
        }
    });
    let rss_file = scratch.join("rss.txt");
    let mut p = Point { n, wall_ms: f64::INFINITY, ..Point::default() };
    for _ in 0..runs.max(1) {
        let m = process::run_measured(&cmd, probe, &rss_file, TIMEOUT, cap)
            .map_err(|e| format!("cannot run {}: {e}", compiler.display()))?;
        let out = m.as_output();
        if !out.success() {
            let stderr = out.stderr_str();
            let head: Vec<&str> = stderr.lines().take(3).collect();
            p.failed = Some(format!("N={n}: {}\n{}", out.describe_end(), head.join("\n")));
            break;
        }
        p.wall_ms = p.wall_ms.min(m.wall.as_secs_f64() * 1000.0);
        p.rss_kb = m.max_rss_kb.max(p.rss_kb);
        if let Some(r) = &report {
            let text = std::fs::read_to_string(r)
                .map_err(|e| format!("kiln wrote no constexpr report ({e})"))?;
            let rep = oven_report::Report::parse(&text)
                .map_err(|e| format!("malformed constexpr report: {e}"))?;
            p.counters = Some(rep.total);
        }
    }
    if p.failed.is_some() {
        p.wall_ms = 0.0;
    }
    let _ = std::fs::remove_file(&file);
    Ok(p)
}

/// The sweep's table, as Markdown (pasted into `docs/status/perf.md`).
pub fn render(points: &[Point]) -> String {
    let mut s = String::from(
        "| N | wall ms | peak RSS MiB | steps | peak_bytes | peak_arena | evaluations |\n\
         |---|---|---|---|---|---|---|\n",
    );
    let c = |p: &Point, f: fn(&Counters) -> u64| {
        p.counters.as_ref().map_or_else(|| "-".to_owned(), |c| f(c).to_string())
    };
    #[allow(clippy::cast_precision_loss)]
    let mib = |p: &Point| {
        p.rss_kb.map_or_else(|| "-".to_owned(), |k| format!("{:.1}", k as f64 / 1024.0))
    };
    for p in points {
        if p.failed.is_some() {
            let _ = writeln!(s, "| {} | failed | - | - | - | - | - |", p.n);
            continue;
        }
        let _ = writeln!(
            s,
            "| {} | {:.0} | {} | {} | {} | {} | {} |",
            p.n,
            p.wall_ms,
            mib(p),
            c(p, |c| c.steps),
            c(p, |c| c.peak_bytes),
            c(p, |c| c.peak_arena),
            c(p, |c| c.evaluations)
        );
    }
    if let [a, b] = points
        && a.failed.is_none()
        && b.failed.is_none()
    {
        #[allow(clippy::cast_precision_loss)]
        let rss = match (a.rss_kb, b.rss_kb) {
            (Some(x), Some(y)) if x > 0 => format!("{:.2}x", y as f64 / x as f64),
            _ => "-".to_owned(),
        };
        let _ = writeln!(s, "| 4N/N | {:.2}x | {rss} | | | | |", b.wall_ms / a.wall_ms);
    }
    s
}

/// The default scratch directory of a sweep.
pub fn scratch_dir(target: &Path) -> PathBuf {
    target.join("xtask-bench").join(format!("scaling-{}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(n: usize, wall_ms: f64, rss_kb: u64, peak_arena: u64) -> Point {
        Point {
            n,
            wall_ms,
            rss_kb: Some(rss_kb),
            counters: Some(Counters { peak_arena, ..Counters::default() }),
            failed: None,
        }
    }

    /// The §3.8 gate: 4.6x on wall and RSS, flat `peak_arena` where the
    /// shape's evaluations do not depend on N.
    #[test]
    fn scaling_gate() {
        let a = point(100, 100.0, 1000, 512);
        assert_eq!(judge("s1", &a, &point(400, 400.0, 4000, 512)), Vec::<String>::new());
        let v = judge("s1", &a, &point(400, 470.0, 4700, 1024));
        assert_eq!(v.len(), 3, "{v:?}");
        assert!(v[0].starts_with("wall time grows 4.70x"), "{v:?}");
        assert!(v[2].contains("peak_arena grows from 512 to 1024"), "{v:?}");
        // S3 and S6 evaluations grow with N by design.
        assert_eq!(judge("s3", &a, &point(400, 400.0, 4000, 4096)), Vec::<String>::new());
        let table = render(&[a.clone(), point(400, 300.0, 2000, 512)]);
        assert!(table.contains("| 100 | 100 | 1.0 | 0 | 0 | 512 | 0 |"), "{table}");
        assert!(table.contains("| 4N/N | 3.00x | 2.00x |"), "{table}");
    }

    /// A sweep under the fake GCC: both sizes compile, the gate runs.
    #[test]
    fn sweep_with_a_fake_compiler() {
        let cc = crate::util::repo_root().join("xtask/fixtures/fake-gcc/g++");
        let Ok(id) = crate::util::identify(&cc) else { return };
        if crate::util::identify(Path::new("clang++-19")).is_err() {
            return;
        }
        let target = crate::util::target_dir(&crate::util::repo_root()).unwrap();
        let scratch = scratch_dir(&target).with_extension("test");
        // A shape the fake (clang 19 underneath) cannot compile: a failure,
        // not an xfail, because the compiler is not kiln.
        let opts = Options {
            scaling: Some("s6".into()),
            scaling_n: Some(3),
            runs: 1,
            ..Options::default()
        };
        assert_eq!(run(&opts, &scratch, &cc, &id), Ok(false));
        let bad = Options { scaling: Some("s99".into()), ..opts };
        assert!(run(&bad, &scratch, &cc, &id).unwrap_err().contains("unknown --scaling shape"));
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
