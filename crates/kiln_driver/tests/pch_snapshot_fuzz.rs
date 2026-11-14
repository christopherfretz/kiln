//! WP-1396: corruption fuzzers of the PCH snapshot consumer (spec/snap.md
//! "Threat model"; EXPERIMENTAL, behind `-fkiln-pch-snapshot=load`).
//!
//! For any bytes presented as a PCH, kiln neither panics, nor aborts, nor
//! hangs, nor allocates without bound: it refuses the snapshot (and
//! replays the header) or reports a clean error. Two models, each over a
//! C++ and a C prefix ([cpp.include]):
//!
//! - **raw** (accidental damage): the whole file is mutated. Every
//!   checksum stays as written, so the damage must be caught: every
//!   mutant is a clean error, except one whose bytes are the intact
//!   file's (a byte run that wrote what was there), which restores.
//! - **crafted**: the snapshot section (one mutant in eight: only its
//!   lookup log) is mutated and the file re-framed
//!   with every checksum recomputed (`kiln_driver::pch_fuzz`), so the
//!   damage reaches the snapshot's decoders. A crafted file can change
//!   what a restored compile means (a flipped constant is a valid
//!   constant), so a different successful output is counted, not failed.
//!   The decoders' known gap (spec/snap.md "Threat model" 3: an internal
//!   compiler error, a `SIGABRT` abort, a hang) is reached by about 2% of
//!   crafted mutants wherever the layout puts the bytes, so it is counted
//!   and printed by site, not failed; any other signal fails.
//!   `KILN_PCH_FUZZ_STRICT=1` fails the known gap too (for the work that
//!   closes it).
//!
//! The mutations: bit flips, byte runs, truncation (at every boundary of
//! the file's framing and at random offsets), swaps of two ranges, and
//! edits of length-like fields (a `u32` or LEB128 at a random offset
//! scaled, offset or saturated). The PRNG is a fixed-seed splitmix64.
//!
//! `framing_and_streamed_body_damage` aims at the paths the consumer
//! reads on its own: every byte of the section block's framing (the
//! section table) and bytes of the snapshot body that only the streaming
//! decode's in-stream checksum sees.
//!
//! The default budget fits CI (about 300 mutants over the two prefixes).
//! A longer local run: `KILN_PCH_FUZZ_ITERS=<n>` mutants per model and
//! prefix, `KILN_PCH_FUZZ_SEED=<u64>`, `KILN_PCH_FUZZ_CASE=<dir>` (a
//! `scripts/pch-snapshot-diff/<case>` directory: `prefix.h`, `main.cpp`
//! or `main.c`, `flags`) in place of the built-in prefixes,
//! `KILN_PCH_FUZZ_THREADS=<n>`, and `KILN_PCH_FUZZ_FAST=1`, which stops a
//! consumer as soon as it reports that the snapshot is not used (its
//! replay is the ordinary path, checked by the default run); every
//! outcome is counted and printed (`--nocapture`).
#![forbid(unsafe_code)]
#![allow(
    clippy::many_single_char_names,
    reason = "d(ir), o(utput), e(rr), b(ytes), n, x, y: short-lived locals"
)]

use std::collections::BTreeMap;
use std::io::{BufRead as _, BufReader};
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn kiln_bin(cxx: bool) -> PathBuf {
    let k = PathBuf::from(env!("CARGO_BIN_EXE_kiln"));
    if cxx { k.with_file_name("kiln++") } else { k }
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-pchfuzz-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn env_num(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

/// splitmix64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (`n > 0`).
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Mutation {
    BitFlip,
    ByteRun,
    Truncate,
    Swap,
    Length,
}

const MUTATIONS: [Mutation; 5] =
    [Mutation::BitFlip, Mutation::ByteRun, Mutation::Truncate, Mutation::Swap, Mutation::Length];

/// Applies `m` to `b`; `cuts` are the offsets a truncation prefers (the
/// framing's boundaries). Always changes `b` when it is not empty.
fn mutate(rng: &mut Rng, b: &mut Vec<u8>, m: Mutation, cuts: &[usize]) {
    if b.is_empty() {
        return;
    }
    let n = b.len();
    match m {
        Mutation::BitFlip => {
            for _ in 0..=rng.below(3) {
                let at = rng.below(n);
                b[at] ^= 1 << rng.below(8);
            }
        }
        Mutation::ByteRun => {
            let at = rng.below(n);
            let len = (1 + rng.below(32)).min(n - at);
            let fill = rng.below(4);
            for i in 0..len {
                let v = match fill {
                    0 => 0,
                    1 => 0xff,
                    2 => rng.next() as u8,
                    _ => b[rng.below(n)],
                };
                b[at + i] = v;
            }
        }
        Mutation::Truncate => {
            let at = if !cuts.is_empty() && rng.below(2) == 0 {
                let c = cuts[rng.below(cuts.len())];
                // At the boundary, or a byte either side of it.
                (c + rng.below(3)).saturating_sub(1).min(n - 1)
            } else {
                rng.below(n)
            };
            if rng.below(4) == 0 && at + 1 < n {
                // Cut a piece out of the middle instead.
                let end = (at + 1 + rng.below(4096)).min(n);
                b.drain(at..end);
            } else {
                b.truncate(at);
            }
        }
        Mutation::Swap => {
            if n < 2 {
                b[0] ^= 1;
                return;
            }
            let len = (1 + rng.below(256)).min(n / 2);
            let x = rng.below(n - 2 * len + 1);
            let y = x + len + rng.below(n - x - 2 * len + 1);
            let tmp = b[x..x + len].to_vec();
            b.copy_within(y..y + len, x);
            b[y..y + len].copy_from_slice(&tmp);
            if b[x..x + len] == tmp[..] {
                b[x] ^= 0x80;
            }
        }
        Mutation::Length => {
            let at = rng.below(n);
            if rng.below(2) == 0 && at + 4 <= n {
                let old = u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
                let new = match rng.below(6) {
                    0 => old.wrapping_add(1),
                    1 => old.wrapping_sub(1),
                    2 => old.wrapping_mul(2).max(old.wrapping_add(2)),
                    3 => u32::MAX,
                    4 => old ^ 0x8000_0000,
                    _ => old.wrapping_add(1 + rng.below(1 << 16) as u32),
                };
                b[at..at + 4].copy_from_slice(&new.to_le_bytes());
            } else {
                // A LEB128 length: make its byte longer (continuation bit)
                // or its value larger.
                b[at] = if b[at] & 0x80 == 0 { b[at] | 0x80 } else { b[at].wrapping_add(0x10) };
            }
        }
    }
}

/// A prefix and a main file compiled with it.
struct Case {
    name: String,
    flags: Vec<String>,
    header: String,
    main: String,
    main_name: &'static str,
    c: bool,
}

const CXX_PREFIX: &str = r#"#define SCALE 3
#define STR(x) #x
struct Base { virtual int v() const { return 1; } virtual ~Base() = default; };
struct Der : Base { int v() const override { return 2; } };
constexpr int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
constexpr int table[] = { fib(5), fib(10), fib(15) };
template <class T> struct Box { T t; constexpr T get() const { return t * SCALE; } };
constexpr int sum_new(int n) {
  int *p = new int[n];
  for (int i = 0; i < n; ++i) p[i] = i;
  int s = 0;
  for (int i = 0; i < n; ++i) s += p[i];
  delete[] p;
  return s;
}
static_assert(sum_new(10) == 45);
struct Str { const char *s; constexpr unsigned len() const { unsigned n = 0; while (s[n]) ++n; return n; } };
constexpr Str hello{"hello"};
inline int counter = 0;
enum class Color : unsigned char { red = 1, green = 4 };
namespace ns { template <class T, int N> constexpr T scaled(T v) { return v * N; } }
#line 100 "renamed.h"
template <class T> int needs_member(T t) { return t.member; }
"#;

const CXX_MAIN: &str = r"constexpr Box<int> b{7};
static_assert(b.get() == 21);
constexpr int f20 = fib(20);
static_assert(hello.len() == 5);
const char *name = STR(SCALE);
int use() {
  Der d;
  Base &r = d;
  return r.v() + table[2] + f20 + sum_new(5) + SCALE + counter + int(Color::green)
      + ns::scaled<int, 4>(2);
}
";

const C_PREFIX: &str = r#"#define MAX(a, b) ((a) > (b) ? (a) : (b))
struct point { int x, y; };
enum shade { DARK = 2, LIGHT = 9 };
typedef struct point point_t;
static inline int dot(point_t a, point_t b) { return a.x * b.x + a.y * b.y; }
int tentative;
_Static_assert(sizeof(struct point) == 2 * sizeof(int), "layout");
static const char greeting[] = "hi";
extern int ext(int);
"#;

const C_MAIN: &str = r"int use(void) {
  point_t p = { 1, 2 }, q = { MAX(3, 4), LIGHT };
  return dot(p, q) + tentative + (int)sizeof greeting;
}
";

fn builtin_cases() -> Vec<Case> {
    vec![
        Case {
            name: "cxx".into(),
            flags: vec!["-std=c++26".into()],
            header: CXX_PREFIX.into(),
            main: CXX_MAIN.into(),
            main_name: "main.cpp",
            c: false,
        },
        Case {
            name: "c".into(),
            flags: vec!["-std=c17".into()],
            header: C_PREFIX.into(),
            main: C_MAIN.into(),
            main_name: "main.c",
            c: true,
        },
    ]
}

/// A `scripts/pch-snapshot-diff/<case>` directory.
fn dir_case(dir: &Path) -> Case {
    let read = |f: &str| std::fs::read_to_string(dir.join(f));
    let c = dir.join("main.c").exists();
    let flags = read("flags").unwrap_or_default();
    Case {
        name: dir.file_name().unwrap().to_string_lossy().into_owned(),
        flags: flags.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect(),
        header: read("prefix.h").expect("prefix.h"),
        main: read(if c { "main.c" } else { "main.cpp" }).expect("main file"),
        main_name: if c { "main.c" } else { "main.cpp" },
        c,
    }
}

/// How one consumer run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Ending {
    /// Exited with `code`: the IR and the diagnostics (timing lines and
    /// `-v` notes left out), and whether the snapshot was restored.
    Exit { code: i32, ir: String, diags: String, restored: bool },
    /// Stopped once it reported the snapshot unused (fast mode).
    Refused,
    /// A panic or an internal compiler error report.
    Panic(String),
    /// Killed by a signal (its number; an abort, a failed allocation):
    /// the status and the last lines.
    Signal(Option<i32>, String),
    /// Did not finish within the time limit.
    Hang,
}

/// Runs the consumer of `pch` in the case's directory `dir` (the
/// producer's working directory, part of the snapshot's identity), its
/// output in `work`.
fn consume(
    case: &Case,
    dir: &Path,
    work: &Path,
    pch: &Path,
    fast: bool,
    limit: Duration,
) -> Ending {
    let out = work.join("out.ll");
    let _ = std::fs::remove_file(&out);
    // Bounded memory: an unbounded allocation fails (an abort) instead of
    // taking the machine.
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("ulimit -v 4000000; exec \"$@\"").arg("sh");
    cmd.arg(kiln_bin(!case.c)).args(&case.flags);
    cmd.arg("-include-pch").arg(pch);
    cmd.args(["-fkiln-pch-snapshot=load", "-fno-crash-diagnostics", "-S", "-emit-llvm", "-o"]);
    cmd.arg(&out).arg(case.main_name);
    if fast {
        cmd.arg("-v");
    }
    let mut child = cmd
        .current_dir(dir)
        .env("KILN_SNAP_TIMING", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (tx, rx) = mpsc::channel::<Option<String>>();
    let err = child.stderr.take().unwrap();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(err).lines() {
            let Ok(line) = line else { break };
            if tx.send(Some(line)).is_err() {
                break;
            }
        }
        let _ = tx.send(None);
    });
    let started = Instant::now();
    let mut lines = Vec::new();
    let mut refused = false;
    loop {
        let left = limit.saturating_sub(started.elapsed());
        match rx.recv_timeout(left.max(Duration::from_millis(1))) {
            Ok(Some(l)) => {
                if fast && l.contains("snapshot not used") {
                    refused = true;
                    let _ = child.kill();
                }
                lines.push(l);
            }
            Ok(None) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // SIGKILL to the compiler itself (`sh` exec'd it). The
                // reader is not joined: nothing still holding the pipe can
                // stall the test past the limit.
                let _ = child.kill();
                let _ = child.wait();
                drop(reader);
                return Ending::Hang;
            }
        }
    }
    let status = child.wait().unwrap();
    let _ = reader.join();
    let text = lines.join("\n");
    if let Some(at) = text.find("panicked").or_else(|| text.find("internal compiler error")) {
        // From the panic's line: `-v`'s banner would fill the message.
        let line = text[..at].rfind('\n').map_or(0, |i| i + 1);
        return Ending::Panic(text[line..].to_owned());
    }
    if refused {
        return Ending::Refused;
    }
    let Some(code) = status.code() else {
        let tail = lines[lines.len().saturating_sub(8)..].join("\n");
        return Ending::Signal(status.signal(), format!("{status}: {tail}"));
    };
    let restored = lines.iter().any(|l| l.starts_with("snapshot-timing: restore "));
    // `-v` (fast mode) prints its banner and search list first.
    let banner = if fast {
        lines.iter().position(|l| l.starts_with("End of search list")).map_or(0, |i| i + 1)
    } else {
        0
    };
    let diags = lines[banner..]
        .iter()
        .filter(|l| {
            // `-v`'s search-list entries and echoed commands go; caret
            // snippets stay.
            let search_entry = l.starts_with(' ') && l.trim_start().starts_with(['/', '"']);
            !(search_entry
                || l.starts_with("snapshot-timing: ")
                || l.starts_with("note: precompiled header")
                || l.starts_with('#')
                || l.starts_with("End of search list"))
        })
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let ir = std::fs::read_to_string(&out).unwrap_or_default();
    Ending::Exit { code, ir, diags, restored }
}

/// An ending without its IR, for messages.
fn short(e: &Ending) -> String {
    match e {
        Ending::Exit { code, diags, restored, ir } => format!(
            "exit {code}, restored {restored}, {} bytes of IR, diagnostics: {}",
            ir.len(),
            diags.chars().take(400).collect::<String>()
        ),
        other => format!("{other:?}").chars().take(400).collect(),
    }
}

/// The outcome classes counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    /// Restored, and the output is replay's.
    RestoredSame,
    /// Not restored (replayed), and the output is replay's.
    Refused,
    /// A clean error (a nonzero exit without a panic).
    CleanError,
    /// Compiled successfully with another output (crafted model only).
    Different,
    /// Failures.
    Panic,
    Signal,
    Hang,
}

/// What a mutant's outcome means for the test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    Passes,
    /// The known gap (spec/snap.md "Threat model" 3): counted, not failed
    /// unless `KILN_PCH_FUZZ_STRICT` is set.
    Tolerated,
    Fails,
}

/// `KILN_PCH_FUZZ_STRICT=1`: the known gap fails too (for the work that
/// closes it).
fn strict() -> bool {
    std::env::var_os("KILN_PCH_FUZZ_STRICT").is_some_and(|v| !v.is_empty() && v != "0")
}

/// A raw (checksums as written) mutant must be a clean error, or, when
/// its bytes are the intact file's, the intact restore. Independent of
/// the layout: the checksums cover every byte.
fn raw_verdict(class: Class, no_op: bool) -> Verdict {
    match class {
        Class::CleanError => Verdict::Passes,
        Class::RestoredSame if no_op => Verdict::Passes,
        _ => Verdict::Fails,
    }
}

/// A crafted (checksums recomputed) mutant may reach the decoders' known
/// gap, whichever bytes the layout puts where: an internal compiler error
/// through the crash handler, an abort (`SIGABRT`: a stack overflow or a
/// failed allocation under the memory cap), or a hang (killed at the
/// limit). Those are tolerated by default. Any other signal (with
/// `forbid(unsafe_code)`, a `SIGSEGV` or `SIGBUS` is not the known gap)
/// fails. So does a harness error (a panic of the test itself). A
/// restore with another output is a valid crafted outcome and passes.
fn crafted_verdict(end: &Ending, class: Class) -> Verdict {
    let gap = match end {
        Ending::Panic(_) | Ending::Hang => true,
        Ending::Signal(sig, _) => *sig == Some(6),
        Ending::Exit { .. } | Ending::Refused => return Verdict::Passes,
    };
    debug_assert!(matches!(class, Class::Panic | Class::Signal | Class::Hang));
    if gap && !strict() { Verdict::Tolerated } else { Verdict::Fails }
}

/// Where a tolerated outcome comes from: the compiler source an internal
/// error names, or the kind.
fn site(end: &Ending) -> String {
    match end {
        Ending::Panic(t) => {
            t.split("in compiler source ").nth(1).and_then(|s| s.split(' ').next()).map_or_else(
                || "internal error".to_owned(),
                |s| s.rsplit_once(':').map_or(s, |x| x.0).to_owned(),
            )
        }
        Ending::Signal(sig, _) => format!("signal {sig:?}"),
        Ending::Hang => "hang".to_owned(),
        Ending::Exit { .. } | Ending::Refused => "other".to_owned(),
    }
}

struct Report {
    /// The tolerated outcomes, by [`site`].
    tolerated: BTreeMap<String, usize>,
    counts: BTreeMap<(&'static str, Class), usize>,
    by_mutation: BTreeMap<(&'static str, Mutation, Class), usize>,
    failures: Vec<String>,
}

#[allow(clippy::too_many_lines)]
fn fuzz_case(
    case: &Case,
    iters: u64,
    seed: u64,
    threads: usize,
    fast: bool,
    report: &Mutex<Report>,
) {
    let d = scratch(&case.name);
    std::fs::write(d.join("prefix.h"), &case.header).unwrap();
    std::fs::write(d.join(case.main_name), &case.main).unwrap();
    let lang = if case.c { "c-header" } else { "c++-header" };
    let o = Command::new(kiln_bin(!case.c))
        .args(&case.flags)
        .args(["-fkiln-pch-snapshot=write", "-fno-crash-diagnostics", "-v", "-x", lang])
        .args(["prefix.h", "-o", "p.pch"])
        .current_dir(&d)
        .output()
        .unwrap();
    let e = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success() && e.contains("snapshot written"), "{}: {e}", case.name);
    let pch = std::fs::read(d.join("p.pch")).unwrap();
    let layout = kiln_driver::pch_fuzz::layout(&pch).unwrap();
    let snap = layout.snapshot().expect("a snapshot section");
    let limit = Duration::from_secs(env_num("KILN_PCH_FUZZ_TIMEOUT").unwrap_or(60));
    // The reference: the intact PCH replayed; and its restore agrees.
    let reference = {
        let mut cmd = Command::new(kiln_bin(!case.c));
        cmd.args(&case.flags).args(["-include-pch", "p.pch", "-fno-crash-diagnostics"]);
        cmd.args(["-S", "-emit-llvm", "-o", "ref.ll"]);
        let o = cmd.arg(case.main_name).current_dir(&d).output().unwrap();
        let diags = String::from_utf8_lossy(&o.stderr).trim_end().to_owned();
        let ir = std::fs::read_to_string(d.join("ref.ll")).unwrap();
        Ending::Exit { code: o.status.code().unwrap(), ir, diags, restored: false }
    };
    let intact = consume(case, &d, &d, &d.join("p.pch"), false, limit);
    match (&intact, &reference) {
        (
            Ending::Exit { code, ir, diags, restored: true },
            Ending::Exit { code: c0, ir: i0, diags: d0, .. },
        ) => {
            assert_eq!((code, diags, ir == i0), (c0, d0, true), "{}: restore vs replay", case.name);
        }
        _ => panic!("{}: the intact snapshot was not restored: {}", case.name, short(&intact)),
    }
    let cuts = layout.boundaries();
    let snap_bytes = pch[snap.clone()].to_vec();
    let log_at = kiln_driver::pch_fuzz::lookup_log_range(&snap_bytes).unwrap();
    // Truncation points of the snapshot alone: random, plus its start.
    let snap_cuts = vec![0, snap_bytes.len() / 2];
    let next = Mutex::new(0u64);
    let total = 2 * iters;
    std::thread::scope(|s| {
        for t in 0..threads {
            let (d, next, cuts, snap_cuts, snap_bytes, pch, log_at) =
                (&d, &next, &cuts, &snap_cuts, &snap_bytes, &pch, &log_at);
            let reference = &reference;
            s.spawn(move || {
                let td = d.join(format!("t{t}"));
                std::fs::create_dir_all(&td).unwrap();
                loop {
                    let i = {
                        let mut n = next.lock().unwrap();
                        if *n >= total {
                            break;
                        }
                        *n += 1;
                        *n - 1
                    };
                    // Mutant `i` is fully determined by the seed and `i`.
                    let crafted = i % 2 == 1;
                    let mut rng = Rng(seed ^ i.wrapping_mul(0xa076_1d64_78bd_642f));
                    let m = MUTATIONS[rng.below(MUTATIONS.len())];
                    let bytes = if crafted {
                        let mut snap = snap_bytes.clone();
                        if rng.below(8) == 0 {
                            // One in eight within the head's lookup log
                            // (WP-1403), spliced back (it may change size).
                            let mut part = snap[log_at.clone()].to_vec();
                            mutate(&mut rng, &mut part, m, &[]);
                            snap.splice(log_at.clone(), part);
                        } else {
                            mutate(&mut rng, &mut snap, m, snap_cuts);
                        }
                        kiln_driver::pch_fuzz::edit_snapshot(pch, &mut |b| b.clone_from(&snap))
                            .unwrap()
                    } else {
                        let mut b = pch.clone();
                        mutate(&mut rng, &mut b, m, cuts);
                        b
                    };
                    let path = td.join("p.pch");
                    std::fs::write(&path, &bytes).unwrap();
                    let end = consume(case, d, &td, &path, fast, limit);
                    let model = if crafted { "crafted" } else { "raw" };
                    let class = match &end {
                        Ending::Refused => Class::Refused,
                        Ending::Panic(_) => Class::Panic,
                        Ending::Signal(..) => Class::Signal,
                        Ending::Hang => Class::Hang,
                        Ending::Exit { code, ir, diags, restored } => {
                            let Ending::Exit { code: c0, ir: i0, diags: d0, .. } = reference
                            else {
                                unreachable!()
                            };
                            if code == c0 && ir == i0 && diags == d0 {
                                if *restored { Class::RestoredSame } else { Class::Refused }
                            } else if *code != 0 {
                                Class::CleanError
                            } else {
                                Class::Different
                            }
                        }
                    };
                    let verdict = if crafted {
                        crafted_verdict(&end, class)
                    } else {
                        raw_verdict(class, bytes == *pch)
                    };
                    let mut r = report.lock().unwrap();
                    *r.counts.entry((model, class)).or_default() += 1;
                    *r.by_mutation.entry((model, m, class)).or_default() += 1;
                    if verdict == Verdict::Tolerated {
                        *r.tolerated.entry(site(&end)).or_default() += 1;
                    }
                    if verdict == Verdict::Fails {
                        let keep = d.join(format!("fail-{model}-{i}.pch"));
                        let _ = std::fs::write(&keep, &bytes);
                        let what = match &end {
                            Ending::Panic(t) | Ending::Signal(_, t) => {
                                t.lines().take(6).collect::<Vec<_>>().join(" | ")
                            }
                            other => short(other),
                        };
                        r.failures.push(format!(
                            "{} {model} mutant {i} ({m:?}, seed {seed}): {class:?}: {what} (kept as {})",
                            case.name,
                            keep.display()
                        ));
                    }
                    let _ = std::fs::remove_file(&path);
                }
            });
        }
    });
    let failed = !report.lock().unwrap().failures.is_empty();
    if !failed {
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// The corruption fuzz (see the module docs): every raw mutant is a clean
/// error (or a no-op that restores the intact output); a crafted mutant
/// fails only outside the known gap ([`crafted_verdict`]), and with
/// `KILN_PCH_FUZZ_STRICT=1` on any panic, abort or hang.
#[test]
fn snapshot_corruption_fuzz() {
    let iters = env_num("KILN_PCH_FUZZ_ITERS").unwrap_or(75);
    let seed = env_num("KILN_PCH_FUZZ_SEED").unwrap_or(0x5eed_1396);
    let fast = std::env::var_os("KILN_PCH_FUZZ_FAST").is_some();
    let threads = env_num("KILN_PCH_FUZZ_THREADS").map_or_else(
        || std::thread::available_parallelism().map_or(2, |n| n.get().min(4)),
        |n| n as usize,
    );
    let cases = match std::env::var_os("KILN_PCH_FUZZ_CASE") {
        Some(dir) => vec![dir_case(Path::new(&dir))],
        None => builtin_cases(),
    };
    let report = Mutex::new(Report {
        tolerated: BTreeMap::new(),
        counts: BTreeMap::new(),
        by_mutation: BTreeMap::new(),
        failures: Vec::new(),
    });
    let started = Instant::now();
    for case in &cases {
        fuzz_case(case, iters, seed, threads, fast, &report);
    }
    let r = report.into_inner().unwrap();
    eprintln!(
        "pch-fuzz: {} case(s), {iters} mutants per model each, seed {seed:#x}, {threads} thread(s), {:.1} s",
        cases.len(),
        started.elapsed().as_secs_f64()
    );
    for ((model, class), n) in &r.counts {
        eprintln!("pch-fuzz: {model} {class:?}: {n}");
    }
    for ((model, m, class), n) in &r.by_mutation {
        eprintln!("pch-fuzz:   {model} {m:?} {class:?}: {n}");
    }
    let n: usize = r.tolerated.values().sum();
    eprintln!("pch-fuzz: tolerated (the known gap; KILN_PCH_FUZZ_STRICT=1 fails them): {n}");
    for (at, n) in &r.tolerated {
        eprintln!("pch-fuzz:   {at}: {n}");
    }
    assert!(
        r.failures.is_empty(),
        "{} failing mutant(s):\n{}",
        r.failures.len(),
        r.failures.join("\n")
    );
}

/// Targeted damage to the framing the consumer reads on its own paths
/// (spec/snap.md "Threat model"): every byte of the section block's
/// framing (the section table that `bmi::section_table` searches, before
/// and after the sections' bytes), and single bytes inside the snapshot
/// section's body, which with the streaming decode only the in-stream
/// checksum sees. The checksums are not recomputed: each mutant is refused
/// (replayed), a clean error, or a restore with exactly the replay's
/// output; never a panic, a signal or a hang ([cpp.include]).
#[test]
fn framing_and_streamed_body_damage() {
    let case = &builtin_cases()[0];
    let d = scratch("framing");
    std::fs::write(d.join("prefix.h"), &case.header).unwrap();
    std::fs::write(d.join(case.main_name), &case.main).unwrap();
    let o = Command::new(kiln_bin(true))
        .args(&case.flags)
        .args(["-fkiln-pch-snapshot=write", "-fno-crash-diagnostics", "-v", "-x", "c++-header"])
        .args(["prefix.h", "-o", "p.pch"])
        .current_dir(&d)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let pch = std::fs::read(d.join("p.pch")).unwrap();
    let layout = kiln_driver::pch_fuzz::layout(&pch).unwrap();
    let snap = layout.snapshot().expect("a snapshot section");
    let text_end = *layout.lines.last().unwrap();
    let first = layout.sections.iter().map(|(_, r)| r.start).min().unwrap();
    let last = layout.sections.iter().map(|(_, r)| r.end).max().unwrap();
    let mut spots: Vec<usize> = (text_end..first).chain(last..pch.len()).take(256).collect();
    let n_framing = spots.len();
    spots.extend([snap.start + snap.len() / 3, snap.start + snap.len() / 2, snap.end - 1]);
    let limit = Duration::from_mins(1);
    let reference = consume(case, &d, &d, &d.join("p.pch"), false, limit);
    let Ending::Exit { code: c0, ir: i0, diags: d0, restored: true } = &reference else {
        panic!("the intact snapshot was not restored: {}", short(&reference))
    };
    let mut caught = 0;
    for at in spots {
        let mut b = pch.clone();
        b[at] ^= 0x5a;
        let m = d.join("m.pch");
        std::fs::write(&m, &b).unwrap();
        match consume(case, &d, &d, &m, false, limit) {
            Ending::Exit { restored: false, .. } | Ending::Refused => caught += 1,
            Ending::Exit { code, ir, diags, restored: true } => {
                assert_eq!(
                    (&code, &diags, ir == *i0),
                    (c0, d0, true),
                    "byte {at} restored differently"
                );
            }
            other => panic!("byte {at}: {}", short(&other)),
        }
    }
    eprintln!(
        "pch-fuzz: framing: {n_framing} framing bytes and 3 body bytes, {caught} refused or rejected"
    );
    assert!(caught >= 3, "the body damage was not caught");
    let _ = std::fs::remove_dir_all(&d);
}

/// The section's lookup log (WP-1403), read from the file and repeated by
/// a consumer whose search list differs (spec/pch.md "Lookup-tolerant
/// search path"; spec/snap.md "Threat model"): damage to it is a clean
/// outcome ([cpp.include]). Bytes of the log are flipped as written
/// (raw: the block checksum catches every one, a clean error) and with
/// the checksums recomputed (crafted: the log decodes, or is refused, and
/// is repeated; a flip that moves the log's end moves where the roots are
/// decoded from, so the known gap is tolerated as in the main fuzz unless
/// `KILN_PCH_FUZZ_STRICT=1`). Then well-formed logs with hostile names,
/// directories and indices are encoded in its place: the roots decode as
/// the intact file's, so these are strict (restored, or refused).
#[test]
#[allow(clippy::too_many_lines, reason = "one scenario: three damage models over one log")]
fn lookup_log_damage() {
    let d = scratch("lookups");
    for (f, text) in [
        (
            "prefix.h",
            "#include <a.h>\n#include \"local.h\"\n#if __has_include(<maybe.h>)\n#endif\n",
        ),
        ("local.h", "int local = 2;\n"),
        ("inc/a.h", "int a = 1;\n"),
        ("extra/only.h", "int only = 3;\n"),
        ("main.cpp", "#include <only.h>\nint use() { return a + local + only; }\n"),
    ] {
        let p = d.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    let o = Command::new(kiln_bin(true))
        .args(["-std=c++26", "-Iinc", "-fkiln-pch-snapshot=write", "-fno-crash-diagnostics"])
        .args(["-x", "c++-header", "prefix.h", "-o", "p.pch"])
        .current_dir(&d)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    // The consumer adds a directory: the lookups are repeated.
    let case = Case {
        name: "lookups".into(),
        flags: ["-std=c++26", "-Iinc", "-Iextra"].map(str::to_owned).to_vec(),
        header: String::new(),
        main: String::new(),
        main_name: "main.cpp",
        c: false,
    };
    let limit = Duration::from_mins(1);
    let pch = std::fs::read(d.join("p.pch")).unwrap();
    let snap = kiln_driver::pch_fuzz::layout(&pch).unwrap().snapshot().unwrap();
    let section = pch[snap.clone()].to_vec();
    let log_at = kiln_driver::pch_fuzz::lookup_log_range(&section).unwrap();
    let Ending::Exit { code: c0, ir: i0, diags: d0, restored: true } =
        consume(&case, &d, &d, &d.join("p.pch"), false, limit)
    else {
        panic!("the intact snapshot was not restored over the other search list")
    };
    let mut counts: BTreeMap<(&str, String), usize> = BTreeMap::new();
    let mut run = |model: &'static str, bytes: &[u8], what: &str| {
        let m = d.join("m.pch");
        std::fs::write(&m, bytes).unwrap();
        let end = consume(&case, &d, &d, &m, false, limit);
        let class = match &end {
            Ending::Exit { code, ir, diags, restored } => {
                if code == &c0 && ir == &i0 && diags == &d0 {
                    if *restored { "restored" } else { "refused" }
                } else if *code != 0 {
                    "clean error"
                } else if ir == &i0 && diags.contains("pch-snapshot-unused") {
                    // A log that decodes may name other results: the
                    // snapshot is refused with -Wpch-snapshot-unused.
                    "refused, warned"
                } else {
                    "different"
                }
            }
            _ => "",
        };
        let class = match model {
            // The checksum covers every byte: a clean error, always.
            "raw" => {
                assert_eq!(class, "clean error", "{what} (raw): {}", short(&end));
                class.to_owned()
            }
            // A well-formed log ends where the roots start, and the roots'
            // bytes are the intact file's: they decode as the intact file
            // does, so no outcome of the known gap can arise. Strict.
            "hostile" => {
                assert!(
                    ["restored", "refused", "refused, warned"].contains(&class),
                    "{what}: {}",
                    short(&end)
                );
                class.to_owned()
            }
            // A flipped byte can change where the log ends (a length or a
            // count), and so where the roots are decoded from: the known
            // gap can be reached, as in the main fuzz ([`crafted_verdict`]).
            _ if !class.is_empty() => class.to_owned(),
            _ => {
                let class = match &end {
                    Ending::Panic(_) => Class::Panic,
                    Ending::Signal(..) => Class::Signal,
                    _ => Class::Hang,
                };
                assert_eq!(
                    crafted_verdict(&end, class),
                    Verdict::Tolerated,
                    "{what} (crafted): {}",
                    short(&end)
                );
                format!("tolerated: {}", site(&end))
            }
        };
        *counts.entry((model, class)).or_default() += 1;
    };
    // CI: an even sample of 128 bytes crafted and 32 raw (the block
    // checksum is one check); `KILN_PCH_FUZZ_LOG_ALL=1`: every byte, both.
    let all = std::env::var_os("KILN_PCH_FUZZ_LOG_ALL").is_some();
    let sample = |k: usize| {
        let step = if all { 1 } else { log_at.len().div_ceil(k).max(1) };
        log_at.clone().step_by(step)
    };
    for at in sample(128) {
        let mut s = section.clone();
        s[at] ^= 0x5a;
        let b = kiln_driver::pch_fuzz::edit_snapshot(&pch, &mut |x| x.clone_from(&s)).unwrap();
        run("crafted", &b, &format!("log byte {at}"));
    }
    for at in sample(32) {
        let mut b = pch.clone();
        b[snap.start + at] ^= 0x5a;
        run("raw", &b, &format!("log byte {at}"));
    }
    // Hostile logs in its place.
    let log: kiln_pp::LookupLog = {
        let mut r = kiln_snap::Reader::new(&section[log_at.clone()]);
        r.get().unwrap()
    };
    let deep = "y/".repeat(1 << 14);
    let hostile =
        ["", ".", "..", "/", "../../../../etc/passwd", "/etc/passwd", "a\0b", "\u{e9}", &deep];
    let mut variants: Vec<kiln_pp::LookupLog> = Vec::new();
    for h in hostile {
        let mut v = log.clone();
        v.lookups.iter_mut().for_each(|l| l.name = h.to_owned());
        variants.push(v);
        let mut v = log.clone();
        v.dirs.iter_mut().for_each(|x| *x = h.to_owned());
        variants.push(v);
        let mut v = log.clone();
        for l in &mut v.lookups {
            l.found = kiln_pp::Found::Embed { path: h.to_owned() };
            l.form = kiln_pp::LookupForm::Embed;
        }
        variants.push(v);
    }
    let n = log.dirs.len() as u32;
    for i in [n, u32::MAX] {
        let mut v = log.clone();
        v.list.push(i);
        variants.push(v);
        let mut v = log.clone();
        v.angled_start = i.max(v.list.len() as u32 + 1);
        variants.push(v);
        let mut v = log.clone();
        v.lookups.iter_mut().for_each(|l| l.includer = Some(i));
        variants.push(v);
        let mut v = log.clone();
        for l in &mut v.lookups {
            l.form = kiln_pp::LookupForm::Next;
            l.after = Some(i);
        }
        variants.push(v);
    }
    for (k, v) in variants.iter().enumerate() {
        let mut w = kiln_snap::Writer::new();
        w.put(v);
        let mut s = section[..log_at.start].to_vec();
        s.extend_from_slice(&w.into_bytes());
        s.extend_from_slice(&section[log_at.end..]);
        let b = kiln_driver::pch_fuzz::edit_snapshot(&pch, &mut |x| x.clone_from(&s)).unwrap();
        run("hostile", &b, &format!("hostile log {k}"));
    }
    eprintln!("pch-fuzz: lookup log of {} bytes: {counts:?}", log_at.len());
    let _ = std::fs::remove_dir_all(&d);
}
