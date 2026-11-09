//! `cargo xtask dist`: the release tarballs (spec/harness.md "Dist",
//! spec/driver.md "Installation layout").
//!
//! Builds `kiln_driver` with the `dist` cargo profile and feature, splits
//! the debug information off with `objcopy`, assembles the install layout
//! and writes two deterministic `.tar.xz` archives of it plus
//! `SHA256SUMS`. `--check` then extracts the stripped archive into a fresh
//! directory and smoke-tests it from outside the checkout with every
//! `KILN_*` variable removed from the environment.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::util;

const USAGE: &str = "\
usage: cargo xtask dist [--target <triple>] [--out <dir>] [--check] [--keep]

Builds kiln with the `dist` profile and feature and writes, in <dir>
(default <target-dir>/kiln-dist):

  kiln-<version>-<triple>.tar.xz          the install tree, binaries stripped of
                                          debug info (symbols kept) with a
                                          .gnu_debuglink to kiln.debug
  kiln-<version>-<triple>-symbols.tar.xz  the same tree with the unstripped binary
  SHA256SUMS                              sha256sum of both

Archives are deterministic: sorted entries, owner 0:0, every mtime
$SOURCE_DATE_EPOCH (default: the commit time of HEAD, else 0).

  --target <triple>  cargo's --target (default: the host); the archive names it
  --out <dir>        where the archives go
  --check            then extract the stripped archive and smoke-test it: --version,
                     -print-resource-dir inside the tree, hello world compiled, linked
                     and run, a <meta> reflection TU (needs the GCC 16.1 oracle,
                     KILN_GCC), a constexpr-heavy TU, no build-host path in the binary
                     or any output, and the debuglink matching the symbols archive
  --keep             keep the staging and check directories

environment: OBJCOPY (default objcopy, else llvm-objcopy-19), SOURCE_DATE_EPOCH,
KILN_GIT_SHA (the commit recorded in the binary; default from git)
";

#[derive(Debug, Default)]
struct Opts {
    target: Option<String>,
    out: Option<PathBuf>,
    check: bool,
    keep: bool,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Option<Opts>, String> {
    let mut o = Opts::default();
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
            _ => (a.clone(), None),
        };
        let mut value = |name: &str| {
            inline.clone().or_else(|| args.next()).ok_or_else(|| format!("{name} needs a value"))
        };
        match flag.as_str() {
            "-h" | "--help" => return Ok(None),
            "--target" => o.target = Some(value("--target")?),
            "--out" => o.out = Some(PathBuf::from(value("--out")?)),
            "--check" if inline.is_none() => o.check = true,
            "--keep" if inline.is_none() => o.keep = true,
            _ => return Err(format!("unknown argument `{a}`")),
        }
    }
    Ok(Some(o))
}

pub fn main(args: impl Iterator<Item = String>) -> ExitCode {
    let opts = match parse(args) {
        Ok(Some(o)) => o,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("xtask dist: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask dist: error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// kiln's version: xtask inherits the workspace version, as `kiln_driver`
/// does (spec/driver.md "Version").
const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAJOR: &str = env!("CARGO_PKG_VERSION_MAJOR");

/// The dependency scanner's names beside `kiln` (symlinks to it; the
/// program name picks the mode): `kiln-scan-deps`, and the
/// `clang-scan-deps-<clang-compat major>` / `clang-scan-deps` names
/// `CMake`'s `find_program` looks for in the compiler's directory
/// (spec/modules.md §11.9). `crates/kiln_driver/build.rs` makes the same
/// links in the cargo profile directory.
pub const SCAN_DEPS_NAMES: &[&str] = &["kiln-scan-deps", "clang-scan-deps-22", "clang-scan-deps"];

/// What one dist run produced.
#[derive(Debug)]
struct Built {
    /// `kiln-<version>-<triple>`: the archives' top directory.
    name: String,
    stripped: PathBuf,
    symbols: PathBuf,
}

fn run(opts: &Opts) -> Result<(), String> {
    let root = util::repo_root();
    let target_dir = util::target_dir(&root)?;
    let triple = match &opts.target {
        Some(t) => t.clone(),
        None => host_triple()?,
    };
    let out = opts.out.clone().unwrap_or_else(|| target_dir.join("kiln-dist"));
    let out = absolute(&out)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    let forbidden = forbidden_paths(&root, &target_dir);
    let exe = build(&root, &target_dir, opts.target.as_deref(), &forbidden)?;
    let built = package(&root, &out, &exe, &triple, opts.keep)?;
    if opts.check {
        check(&root, &out, &built, &triple, &forbidden, opts.keep)?;
        eprintln!("xtask dist: check passed");
    }
    Ok(())
}

fn absolute(p: &Path) -> Result<PathBuf, String> {
    std::path::absolute(p).map_err(|e| format!("{}: {e}", p.display()))
}

/// `host:` of `rustc -vV`.
fn host_triple() -> Result<String, String> {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let o =
        Command::new(&rustc).arg("-vV").output().map_err(|e| format!("cannot run rustc: {e}"))?;
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| "`rustc -vV` printed no host triple".to_owned())
}

/// The build host's paths that must never appear in a dist binary or its
/// output: the checkout, the target directory and Cargo's and rustup's
/// homes, each as given and canonicalized. Each is paired with what
/// `--remap-path-prefix` maps it to.
fn forbidden_paths(root: &Path, target_dir: &Path) -> Vec<(String, &'static str)> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".cargo")));
    let rustup_home = std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".rustup")));
    // Order matters for rustc: the last matching remap wins, so the target
    // directory (often inside the checkout) comes after the checkout.
    let mut v: Vec<(String, &'static str)> = Vec::new();
    let mut add = |p: &Path, to: &'static str| {
        for q in [Some(p.to_path_buf()), std::fs::canonicalize(p).ok()].into_iter().flatten() {
            let s = q.display().to_string();
            // A root-level or empty path would match everything.
            if s.len() > 1 && !v.iter().any(|(x, _)| *x == s) {
                v.push((s, to));
            }
        }
    };
    add(root, "/kiln");
    add(target_dir, "/kiln/target");
    if let Some(c) = cargo_home {
        add(&c, "/cargo");
    }
    if let Some(r) = rustup_home {
        add(&r, "/rustup");
    }
    v
}

/// `CARGO_ENCODED_RUSTFLAGS` for the dist build: the caller's flags
/// (`CARGO_ENCODED_RUSTFLAGS`, else `RUSTFLAGS`) plus a
/// `--remap-path-prefix` per build-host path, so that panic locations of
/// dependencies and the debug info name `/kiln`, `/cargo` ... instead.
fn encoded_rustflags(forbidden: &[(String, &'static str)]) -> OsString {
    let mut flags: Vec<String> = match std::env::var("CARGO_ENCODED_RUSTFLAGS") {
        Ok(s) if !s.is_empty() => s.split('\x1f').map(str::to_owned).collect(),
        _ => std::env::var("RUSTFLAGS")
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
    };
    for (from, to) in forbidden {
        flags.push(format!("--remap-path-prefix={from}={to}"));
    }
    flags.join("\x1f").into()
}

/// `cargo build --locked --profile dist -p kiln_driver --features dist`;
/// the path of the `kiln` binary.
fn build(
    root: &Path,
    target_dir: &Path,
    target: Option<&str>,
    forbidden: &[(String, &'static str)],
) -> Result<PathBuf, String> {
    eprintln!(
        "xtask dist: building kiln (cargo build --profile dist -p kiln_driver --features dist)"
    );
    let mut cmd = Command::new(env!("CARGO"));
    cmd.args(["build", "--quiet", "--locked", "--profile", "dist", "-p", "kiln_driver"])
        .args(["--features", "dist"])
        .env_remove("RUSTFLAGS")
        .env("CARGO_ENCODED_RUSTFLAGS", encoded_rustflags(forbidden))
        .current_dir(root);
    if let Some(t) = target {
        cmd.args(["--target", t]);
    }
    let (status, _) = util::process::run_step(&mut cmd, false)
        .map_err(|e| format!("cannot run cargo build: {e}"))?;
    if !status.success() {
        return Err(format!("cargo build --profile dist failed ({status})"));
    }
    let mut dir = target_dir.to_path_buf();
    if let Some(t) = target {
        dir.push(t);
    }
    let exe = dir.join("dist").join(format!("kiln{}", std::env::consts::EXE_SUFFIX));
    exe.is_file().then_some(exe.clone()).ok_or_else(|| format!("{} was not built", exe.display()))
}

/// The objcopy to use: `$OBJCOPY`, else `objcopy`, else `llvm-objcopy-19`.
fn objcopy() -> Result<OsString, String> {
    if let Some(o) = std::env::var_os("OBJCOPY").filter(|o| !o.is_empty()) {
        return Ok(o);
    }
    for cand in ["objcopy", "llvm-objcopy-19", "llvm-objcopy"] {
        if Command::new(cand)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
        {
            return Ok(cand.into());
        }
    }
    Err("no objcopy (set OBJCOPY)".to_owned())
}

/// Runs `cmd`, failing with its stderr.
fn run_ok(cmd: &mut Command) -> Result<std::process::Output, String> {
    let shown = format!("{cmd:?}");
    let o = cmd.output().map_err(|e| format!("cannot run {shown}: {e}"))?;
    if o.status.success() {
        Ok(o)
    } else {
        Err(format!("{shown} failed ({}): {}", o.status, String::from_utf8_lossy(&o.stderr).trim()))
    }
}

/// The archive mtime: `$SOURCE_DATE_EPOCH`, else HEAD's commit time, else 0.
fn source_date_epoch(root: &Path) -> u64 {
    if let Some(e) = std::env::var("SOURCE_DATE_EPOCH").ok().and_then(|s| s.trim().parse().ok()) {
        return e;
    }
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["log", "-1", "--format=%ct"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(0)
}

fn io_err(what: &str, p: &Path) -> impl FnOnce(std::io::Error) -> String {
    let p = p.display().to_string();
    let what = what.to_owned();
    move |e| format!("cannot {what} {p}: {e}")
}

#[cfg(unix)]
fn set_mode(p: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).map_err(io_err("chmod", p))
}

#[cfg(not(unix))]
fn set_mode(_: &Path, _: u32) -> Result<(), String> {
    Ok(())
}

fn mkdir(p: &Path) -> Result<(), String> {
    std::fs::create_dir_all(p).map_err(io_err("create", p))?;
    set_mode(p, 0o755)
}

fn copy_file(from: &Path, to: &Path, mode: u32) -> Result<(), String> {
    std::fs::copy(from, to).map_err(io_err("copy to", to))?;
    set_mode(to, mode)
}

/// Copies the regular files of `from` into `to`, recursively.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    mkdir(to)?;
    let mut entries: Vec<_> = std::fs::read_dir(from)
        .map_err(io_err("read", from))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        let dest = to.join(p.file_name().unwrap_or_default());
        if p.is_dir() {
            copy_tree(&p, &dest)?;
        } else if p.is_file() {
            copy_file(&p, &dest, 0o644)?;
        }
    }
    Ok(())
}

/// The documents installed flat in `share/doc/kiln/`, relative to the
/// checkout. [`install_doc`] rewrites their links for that layout.
pub const DOCS: &[&str] = &[
    "README.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "docs/COMPATIBILITY.md",
    "docs/KNOWN_ISSUES.md",
    "docs/STABILITY.md",
    "docs/USER_GUIDE.md",
];

/// The source repository, for links to files the archive does not ship.
const REPO: &str = "https://github.com/christopherfretz/kiln";

/// The checkout file `rel` (one of [`DOCS`]) as installed: a Markdown
/// document gets [`rewrite_links`], anything else is copied as is.
fn install_doc(root: &Path, rel: &str, doc: &Path) -> Result<(), String> {
    let from = root.join(rel);
    let to = doc.join(from.file_name().unwrap_or_default());
    if Path::new(rel).extension().is_none_or(|e| e != "md") {
        return copy_file(&from, &to, 0o644);
    }
    let text = std::fs::read_to_string(&from).map_err(io_err("read", &from))?;
    let out = rewrite_links(root, rel, &text, VERSION)?;
    std::fs::write(&to, out).map_err(io_err("write", &to))?;
    set_mode(&to, 0o644)
}

/// `text`, the checkout document `src` (relative to `root`), with each
/// relative link (`crate::doclinks::links`) retargeted for the flat
/// `share/doc/kiln/` layout: a link to another of [`DOCS`] becomes its
/// file name, a link to any other checkout file or directory becomes its
/// [`REPO`] URL at the release tag `v<version>` (`raw/` for an HTML
/// `src=`, `tree/` for a directory, `blob/` otherwise). Anchors are kept;
/// URLs and `#anchor` links are left alone. A link to a path the checkout
/// does not have is an error.
pub fn rewrite_links(root: &Path, src: &str, text: &str, version: &str) -> Result<String, String> {
    use crate::doclinks::{is_external, links, resolve, split_anchor};
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for r in links(text) {
        let target = &text[r.clone()];
        let (path, anchor) = split_anchor(target);
        if is_external(target) || path.is_empty() {
            continue;
        }
        let Some(abs) = resolve(src, path) else {
            return Err(format!("{src}: link {target} leaves the checkout"));
        };
        let on_disk = root.join(&abs);
        if !on_disk.exists() {
            return Err(format!("{src}: link {target} names a missing {abs}"));
        }
        let anchor = anchor.map_or(String::new(), |a| format!("#{a}"));
        let new = if DOCS.contains(&abs.as_str()) {
            let base = abs.rsplit('/').next().unwrap_or(&abs);
            format!("{base}{anchor}")
        } else {
            let kind = if text[..r.start].ends_with("src=\"") {
                "raw"
            } else if on_disk.is_dir() {
                "tree"
            } else {
                "blob"
            };
            format!("{REPO}/{kind}/v{version}/{abs}{anchor}")
        };
        out.push_str(&text[last..r.start]);
        out.push_str(&new);
        last = r.end;
    }
    out.push_str(&text[last..]);
    Ok(out)
}

/// Writes the install tree (spec/driver.md "Installation layout") without
/// the `kiln` binary into `prefix`:
///
/// ```text
/// bin/kiln++ -> kiln
/// bin/kiln-scan-deps, bin/clang-scan-deps-22, bin/clang-scan-deps -> kiln
/// lib/kiln/<major>/include/      resource/include (builtin headers)
/// lib/kiln/<major>/predefines/   resource/predefines/*.txt
/// lib/kiln/<major>/share/        resource/share/*.txt (default ignorelists)
/// share/doc/kiln/                DOCS, links rewritten (install_doc)
/// ```
///
/// `resource/include-paths/` and `predefines/structure.diff` are reference
/// data for the tests and the generator, which kiln never reads.
pub fn assemble(root: &Path, prefix: &Path, major: &str) -> Result<(), String> {
    let res = root.join("resource");
    let bin = prefix.join("bin");
    mkdir(&bin)?;
    #[cfg(unix)]
    for name in std::iter::once(&"kiln++").chain(SCAN_DEPS_NAMES) {
        std::os::unix::fs::symlink("kiln", bin.join(name)).map_err(io_err("link", &bin))?;
    }
    let resdir = prefix.join("lib").join("kiln").join(major);
    mkdir(&prefix.join("lib").join("kiln"))?;
    copy_tree(&res.join("include"), &resdir.join("include"))?;
    let pre = resdir.join("predefines");
    mkdir(&pre)?;
    let src = res.join("predefines");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&src)
        .map_err(io_err("read", &src))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .collect();
    files.sort();
    for f in files {
        copy_file(&f, &pre.join(f.file_name().unwrap_or_default()), 0o644)?;
    }
    // WP-1032: the default sanitizer ignorelists clang reads from
    // `<resource-dir>/share/`.
    let share = resdir.join("share");
    mkdir(&share)?;
    let src = res.join("share");
    let mut lists: Vec<PathBuf> = std::fs::read_dir(&src)
        .map_err(io_err("read", &src))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .collect();
    lists.sort();
    for f in lists {
        copy_file(&f, &share.join(f.file_name().unwrap_or_default()), 0o644)?;
    }
    let doc = prefix.join("share").join("doc").join("kiln");
    mkdir(&doc)?;
    for d in DOCS {
        install_doc(root, d, &doc)?;
    }
    let broken = crate::doclinks::check_tree(&doc);
    if !broken.is_empty() {
        return Err(format!("broken links in share/doc/kiln: {}", broken.join(", ")));
    }
    for d in [prefix, &prefix.join("lib"), &prefix.join("share"), &prefix.join("share").join("doc")]
    {
        set_mode(d, 0o755)?;
    }
    Ok(())
}

/// `tar` of `dir/<name>` into `archive` (`.tar.xz`), deterministically:
/// GNU tar with sorted names, owner and group 0, one mtime, normalized
/// modes, and `xz -9` single-threaded (the multi-threaded encoder's block
/// layout depends on the thread count).
pub fn tar_xz(dir: &Path, name: &str, archive: &Path, epoch: u64) -> Result<(), String> {
    let tar = archive.with_extension(""); // x.tar.xz -> x.tar
    let _ = std::fs::remove_file(&tar);
    let _ = std::fs::remove_file(archive);
    run_ok(
        Command::new("tar")
            .args(["--sort=name", "--format=gnu", "--owner=0", "--group=0", "--numeric-owner"])
            .arg(format!("--mtime=@{epoch}"))
            .arg("--mode=u+rwX,go+rX,go-w")
            .arg("-C")
            .arg(dir)
            .arg("-cf")
            .arg(&tar)
            .arg(name),
    )?;
    run_ok(Command::new("xz").args(["-9", "-T1", "-f"]).arg(&tar))?;
    archive.is_file().then_some(()).ok_or_else(|| format!("xz did not write {}", archive.display()))
}

fn file_size(p: &Path) -> u64 {
    std::fs::metadata(p).map_or(0, |m| m.len())
}

fn mib(n: u64) -> String {
    let tenths = n.saturating_mul(10) / (1024 * 1024);
    format!("{}.{} MiB", tenths / 10, tenths % 10)
}

/// Strips, assembles and archives; writes `SHA256SUMS`.
fn package(root: &Path, out: &Path, exe: &Path, triple: &str, keep: bool) -> Result<Built, String> {
    let name = format!("kiln-{VERSION}-{triple}");
    let work = out.join(format!("{name}.work"));
    if work.exists() {
        std::fs::remove_dir_all(&work).map_err(io_err("remove", &work))?;
    }
    mkdir(&work)?;
    let prefix = work.join(&name);
    assemble(root, &prefix, MAJOR)?;
    let kiln = prefix.join("bin").join("kiln");
    let debug = work.join("kiln.debug");
    let objcopy = objcopy()?;
    run_ok(Command::new(&objcopy).arg("--only-keep-debug").arg(exe).arg(&debug))?;
    run_ok(
        Command::new(&objcopy)
            .arg("--strip-debug")
            .arg(format!("--add-gnu-debuglink={}", debug.display()))
            .arg(exe)
            .arg(&kiln),
    )?;
    set_mode(&kiln, 0o755)?;
    let epoch = source_date_epoch(root);
    let stripped = out.join(format!("{name}.tar.xz"));
    tar_xz(&work, &name, &stripped, epoch)?;
    let stripped_bin = file_size(&kiln);
    copy_file(exe, &kiln, 0o755)?;
    let symbols = out.join(format!("{name}-symbols.tar.xz"));
    tar_xz(&work, &name, &symbols, epoch)?;
    let names = [&stripped, &symbols].map(|p| p.file_name().unwrap_or_default().to_owned());
    let o = run_ok(Command::new("sha256sum").args(names).current_dir(out))?;
    std::fs::write(out.join("SHA256SUMS"), &o.stdout).map_err(io_err("write", out))?;
    if !keep {
        std::fs::remove_dir_all(&work).map_err(io_err("remove", &work))?;
    }
    eprintln!(
        "xtask dist: {} ({}; bin/kiln {})\nxtask dist: {} ({}; bin/kiln {})\nxtask dist: {}",
        stripped.display(),
        mib(file_size(&stripped)),
        mib(stripped_bin),
        symbols.display(),
        mib(file_size(&symbols)),
        mib(file_size(exe)),
        out.join("SHA256SUMS").display(),
    );
    Ok(Built { name, stripped, symbols })
}

/// The CRC-32 (IEEE, as zlib) `.gnu_debuglink` records.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// The file name and CRC of a `.gnu_debuglink` section: the name, NUL
/// padding to a multiple of 4, then the CRC little-endian.
pub fn parse_debuglink(section: &[u8]) -> Option<(String, u32)> {
    let nul = section.iter().position(|&b| b == 0)?;
    let name = String::from_utf8(section[..nul].to_vec()).ok()?;
    let crc = section.get(section.len().checked_sub(4)?..)?;
    Some((name, u32::from_le_bytes(crc.try_into().ok()?)))
}

/// The first build-host path (from [`forbidden_paths`]) in `hay`.
pub fn find_forbidden<'a>(hay: &[u8], forbidden: &'a [(String, &'static str)]) -> Option<&'a str> {
    forbidden
        .iter()
        .map(|(p, _)| p.as_str())
        .find(|p| hay.windows(p.len()).any(|w| w == p.as_bytes()))
}

/// The smoke-test sources (`--check`).
const HELLO: &str = "#include <iostream>\nint main() { std::cout << \"hello from kiln\\n\"; }\n";

const META: &str = "\
#include <meta>
#include <string_view>

struct Point { int x; int y; };

consteval std::size_t member_count() {
  return std::meta::nonstatic_data_members_of(^^Point, std::meta::access_context::unchecked())
      .size();
}
static_assert(member_count() == 2);
static_assert(std::meta::identifier_of(^^Point) == \"Point\");

int main() { return member_count() == 2 ? 0 : 1; }
";

const CONSTEXPR: &str = "\
#include <algorithm>
#include <numeric>
#include <vector>

// A sieve over a heap-allocated table, then a sort and a fold over a
// std::vector, all during constant evaluation.
constexpr int count_primes(int n) {
  bool* composite = new bool[n + 1]{};
  int count = 0;
  for (int i = 2; i <= n; ++i) {
    if (composite[i]) continue;
    ++count;
    for (long j = long(i) * i; j <= n; j += i) composite[j] = true;
  }
  delete[] composite;
  return count;
}

constexpr long sorted_checksum(int n) {
  std::vector<int> v;
  for (int i = 0; i < n; ++i) v.push_back((i * 7919) % n);
  std::sort(v.begin(), v.end());
  long sum = 0;
  for (int i = 0; i < n; ++i) sum += long(v[i]) * (i % 3);
  return sum + std::accumulate(v.begin(), v.end(), 0L);
}

constexpr int primes = count_primes(200000);
static_assert(primes == 17984);
constexpr long checksum = sorted_checksum(20000);

int main() { return checksum == sorted_checksum(20000) ? 0 : 1; }
";

/// One smoke-test step's environment: no `KILN_*` variable, the extracted
/// `bin/` first on `PATH`, `cwd` outside the checkout.
fn smoke_cmd(prog: &Path, cwd: &Path, bin: &Path) -> Command {
    let mut cmd = Command::new(prog);
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().starts_with("KILN_") {
            cmd.env_remove(&k);
        }
    }
    let mut path = vec![bin.to_path_buf()];
    path.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
    cmd.env("PATH", std::env::join_paths(path).unwrap_or_default()).current_dir(cwd);
    cmd
}

/// `--check`: see [`USAGE`].
fn check(
    root: &Path,
    out: &Path,
    built: &Built,
    triple: &str,
    forbidden: &[(String, &'static str)],
    keep: bool,
) -> Result<(), String> {
    let base = out.join(format!("check-{}", std::process::id()));
    // The compiles run outside the checkout: beside the extracted trees
    // when the output directory is (a target directory outside the
    // checkout), else in the system temporary directory.
    let outside = |p: &Path| {
        !p.starts_with(root) && std::fs::canonicalize(root).is_ok_and(|r| !p.starts_with(r))
    };
    let cwd = if outside(&base) {
        base.join("cwd")
    } else {
        std::env::temp_dir().join(format!("kiln-dist-check-{}", std::process::id()))
    };
    for d in [&base, &cwd] {
        if d.exists() {
            std::fs::remove_dir_all(d).map_err(io_err("remove", d))?;
        }
        mkdir(d)?;
    }
    let r = check_in(root, &base, &cwd, built, triple, forbidden);
    if !keep {
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&cwd);
    }
    r
}

fn check_in(
    root: &Path,
    base: &Path,
    cwd: &Path,
    built: &Built,
    triple: &str,
    forbidden: &[(String, &'static str)],
) -> Result<(), String> {
    let cwd = std::fs::canonicalize(cwd).map_err(io_err("resolve", cwd))?;
    if cwd.starts_with(root) || std::fs::canonicalize(root).is_ok_and(|r| cwd.starts_with(r)) {
        return Err(format!("the check directory {} is inside the checkout", cwd.display()));
    }
    let stripped_dir = base.join("stripped");
    let symbols_dir = base.join("symbols");
    for (archive, dir) in [(&built.stripped, &stripped_dir), (&built.symbols, &symbols_dir)] {
        mkdir(dir)?;
        run_ok(Command::new("tar").arg("-xJf").arg(archive).arg("-C").arg(dir))?;
    }
    let prefix = std::fs::canonicalize(stripped_dir.join(&built.name))
        .map_err(io_err("resolve", &stripped_dir))?;
    check_layout(&prefix, triple)?;
    let outputs = smoke(&prefix, &cwd)?;

    // No build-host path in the stripped binary, the symbols binary, or
    // any output (the extracted tree and the scratch directory, which may
    // sit inside the target directory, are blanked out first).
    let mut text = String::from_utf8_lossy(&outputs).into_owned();
    for p in [prefix.display().to_string(), cwd.display().to_string()] {
        text = text.replace(&p, "<tmp>");
    }
    if let Some(p) = find_forbidden(text.as_bytes(), forbidden) {
        return Err(format!("the build-host path {p} appears in kiln's output:\n{text}"));
    }
    let kiln = prefix.join("bin").join("kiln");
    let symbols_bin = symbols_dir.join(&built.name).join("bin").join("kiln");
    for b in [&kiln, &symbols_bin] {
        let bytes = std::fs::read(b).map_err(io_err("read", b))?;
        if let Some(p) = find_forbidden(&bytes, forbidden) {
            return Err(format!("the build-host path {p} appears in {}", b.display()));
        }
    }
    eprintln!("xtask dist: check: no build-host path in the binaries or the output: ok");
    check_debuglink(base, &kiln, &symbols_bin)
}

/// The extracted tree has the install layout.
fn check_layout(prefix: &Path, triple: &str) -> Result<(), String> {
    let bin = prefix.join("bin");
    let resdir = prefix.join("lib").join("kiln").join(MAJOR);
    for p in [
        bin.join("kiln"),
        resdir.join("include").join("stddef.h"),
        resdir.join("predefines").join(format!("{triple}.c++26.txt")),
        prefix.join("share").join("doc").join("kiln").join("STABILITY.md"),
        prefix.join("share").join("doc").join("kiln").join("USER_GUIDE.md"),
    ] {
        if !p.is_file() {
            return Err(format!("the archive has no {}", p.display()));
        }
    }
    let broken = crate::doclinks::check_tree(&prefix.join("share").join("doc").join("kiln"));
    if !broken.is_empty() {
        return Err(format!("the archive has broken doc links: {}", broken.join(", ")));
    }
    for name in std::iter::once(&"kiln++").chain(SCAN_DEPS_NAMES) {
        let link = std::fs::read_link(bin.join(name)).map_err(io_err("read link", &bin))?;
        if link != Path::new("kiln") {
            return Err(format!("bin/{name} points to {}, not kiln", link.display()));
        }
    }
    Ok(())
}

/// The smoke steps, run in `cwd` with the installed `kiln++` found on
/// `PATH`; everything they printed.
fn smoke(prefix: &Path, cwd: &Path) -> Result<Vec<u8>, String> {
    let bin = prefix.join("bin");
    let resdir = prefix.join("lib").join("kiln").join(MAJOR);
    let mut outputs: Vec<u8> = Vec::new();
    let mut step = |what: &str, cmd: &mut Command| -> Result<String, String> {
        let o = run_ok(cmd).map_err(|e| format!("{what}: {e}"))?;
        outputs.extend_from_slice(&o.stdout);
        outputs.extend_from_slice(&o.stderr);
        eprintln!("xtask dist: check: {what}: ok");
        Ok(String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let kilnpp = Path::new("kiln++"); // found on PATH
    let v = step("kiln++ --version", smoke_cmd(kilnpp, cwd, &bin).arg("--version"))?;
    if !v.contains(VERSION) {
        return Err(format!("`kiln++ --version` does not name {VERSION}: {v}"));
    }
    // -print-resource-dir through PATH, the bin/ symlink and a symlink to
    // that symlink elsewhere.
    let other = cwd.join("my-kiln++");
    #[cfg(unix)]
    std::os::unix::fs::symlink(bin.join("kiln++"), &other).map_err(io_err("link", &other))?;
    for prog in [kilnpp, &bin.join("kiln"), &other] {
        let r = step(
            &format!("{} -print-resource-dir", prog.display()),
            smoke_cmd(prog, cwd, &bin).arg("-print-resource-dir"),
        )?;
        if Path::new(r.trim_end()) != resdir {
            return Err(format!(
                "`{} -print-resource-dir` printed {}, not {}",
                prog.display(),
                r.trim_end(),
                resdir.display()
            ));
        }
    }
    for (name, text) in [("hello.cpp", HELLO), ("constexpr.cpp", CONSTEXPR), ("meta.cpp", META)] {
        let p = cwd.join(name);
        std::fs::write(&p, text).map_err(io_err("write", &p))?;
    }
    let cpp = |args: &[&str]| {
        let mut c = smoke_cmd(kilnpp, cwd, &bin);
        c.args(args);
        c
    };
    step("compile and link hello.cpp", &mut cpp(&["hello.cpp", "-o", "hello"]))?;
    let h = step("run hello", &mut smoke_cmd(&cwd.join("hello"), cwd, &bin))?;
    if h != "hello from kiln\n" {
        return Err(format!("hello printed {h:?}"));
    }
    step(
        "compile constexpr.cpp",
        &mut cpp(&[
            "-std=c++26",
            "-O1",
            "-fconstexpr-steps=100000000",
            "constexpr.cpp",
            "-o",
            "constexpr",
        ]),
    )?;
    step("run constexpr", &mut smoke_cmd(&cwd.join("constexpr"), cwd, &bin))?;
    // The scanner alias CMake finds beside the compiler (spec/modules.md
    // §11.9).
    let m = cwd.join("m.cppm");
    std::fs::write(&m, "export module m;\nimport n;\n").map_err(io_err("write", &m))?;
    let scan = step(
        "clang-scan-deps-22 -format=p1689",
        smoke_cmd(Path::new("clang-scan-deps-22"), cwd, &bin).args([
            "-format=p1689",
            "--",
            "kiln++",
            "-std=c++20",
            "-c",
            "m.cppm",
            "-o",
            "m.o",
        ]),
    )?;
    if !scan.contains("\"logical-name\": \"m\"") || !scan.contains("\"logical-name\": \"n\"") {
        return Err(format!("the scan of m.cppm printed {scan:?}"));
    }
    let (gcc, _) = crate::oracle::gcc_from(std::env::var(crate::oracle::GCC_ENV).ok().as_deref());
    match crate::oracle::gcc_toolchain_of(&gcc) {
        Some(tc) => {
            let tc = format!("--gcc-toolchain={}", tc.display());
            step(
                "compile meta.cpp (<meta>, -freflection)",
                &mut cpp(&["-std=c++26", "-freflection", &tc, "-c", "meta.cpp", "-o", "meta.o"]),
            )?;
        }
        None if crate::oracle::require_gcc_from_env() => {
            return Err(format!(
                "no GCC 16.1 at {} for the <meta> step (KILN_REQUIRE_GCC=1)",
                gcc.display()
            ));
        }
        None => eprintln!(
            "xtask dist: check: SKIP the <meta> step: no GCC 16.1 oracle ({}; set KILN_GCC)",
            gcc.display()
        ),
    }
    Ok(outputs)
}

/// The stripped binary's `.gnu_debuglink` names `kiln.debug` with the CRC
/// of `objcopy --only-keep-debug` of the symbols archive's binary, so the
/// debug file a user regenerates from that archive is found by gdb.
fn check_debuglink(base: &Path, kiln: &Path, symbols_bin: &Path) -> Result<(), String> {
    let objcopy = objcopy()?;
    let section = base.join("debuglink");
    run_ok(
        Command::new(&objcopy)
            .arg(format!("--dump-section=.gnu_debuglink={}", section.display()))
            .arg(kiln)
            .arg(base.join("kiln.dump")),
    )?;
    let raw = std::fs::read(&section).map_err(io_err("read", &section))?;
    let (name, crc) = parse_debuglink(&raw).ok_or("malformed .gnu_debuglink")?;
    let debug = base.join("kiln.debug");
    run_ok(Command::new(&objcopy).arg("--only-keep-debug").arg(symbols_bin).arg(&debug))?;
    let want = crc32(&std::fs::read(&debug).map_err(io_err("read", &debug))?);
    if name != "kiln.debug" || crc != want {
        return Err(format!(
            ".gnu_debuglink is {name:?} crc {crc:08x}; expected \"kiln.debug\" crc {want:08x}"
        ));
    }
    eprintln!("xtask dist: check: .gnu_debuglink matches the symbols archive: ok");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options() {
        let p = |v: &[&str]| parse(v.iter().map(|s| (*s).to_owned()));
        let o =
            p(&["--target", "x86_64-unknown-linux-gnu", "--out=d", "--check"]).unwrap().unwrap();
        assert_eq!(o.target.as_deref(), Some("x86_64-unknown-linux-gnu"));
        assert_eq!(o.out.as_deref(), Some(Path::new("d")));
        assert!(o.check && !o.keep);
        assert!(p(&["--help"]).unwrap().is_none());
        assert!(p(&["--target"]).is_err());
        assert!(p(&["--bogus"]).is_err());
    }

    /// The `.gnu_debuglink` CRC is zlib's CRC-32.
    #[test]
    fn crc32_and_debuglink() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
        let mut s = b"kiln.debug\0\0".to_vec();
        s.extend_from_slice(&0x1234_5678u32.to_le_bytes());
        assert_eq!(parse_debuglink(&s), Some(("kiln.debug".to_owned(), 0x1234_5678)));
        assert_eq!(parse_debuglink(b"abc"), None);
    }

    #[test]
    fn forbidden_strings() {
        let f = vec![("/build/kiln".to_owned(), "/kiln")];
        assert_eq!(find_forbidden(b"x /build/kiln/crates y", &f), Some("/build/kiln"));
        assert_eq!(find_forbidden(b"/kiln/crates", &f), None);
    }

    /// spec/harness.md "Dist": the install tree has the layout of
    /// spec/driver.md "Installation layout", and two archives of the same
    /// tree are byte-identical (sorted entries, fixed owner and mtime).
    #[test]
    fn layout_and_deterministic_archive() {
        let root = util::repo_root();
        let d = std::env::temp_dir().join(format!("xtask-dist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let prefix = d.join("kiln-x");
        assemble(&root, &prefix, "7").unwrap();
        for f in [
            "lib/kiln/7/include/stddef.h",
            "lib/kiln/7/include/module.modulemap",
            "lib/kiln/7/include/LICENSE.TXT",
            "lib/kiln/7/predefines/aarch64-unknown-linux-gnu.c++26.txt",
            "lib/kiln/7/predefines/x86_64-unknown-linux-gnu.c17.txt",
            "lib/kiln/7/share/asan_ignorelist.txt",
            "share/doc/kiln/README.md",
            "share/doc/kiln/LICENSE-MIT",
            "share/doc/kiln/LICENSE-APACHE",
            "share/doc/kiln/COMPATIBILITY.md",
            "share/doc/kiln/STABILITY.md",
            "share/doc/kiln/USER_GUIDE.md",
            "share/doc/kiln/KNOWN_ISSUES.md",
            "share/doc/kiln/CHANGELOG.md",
            "share/doc/kiln/CONTRIBUTING.md",
            "share/doc/kiln/SECURITY.md",
        ] {
            assert!(prefix.join(f).is_file(), "{f}");
        }
        // The installed docs link only to each other or to absolute URLs
        // (WP-1027): the README's docs/ links are flattened.
        let doc = prefix.join("share/doc/kiln");
        assert_eq!(crate::doclinks::check_tree(&doc), Vec::<String>::new());
        let readme = std::fs::read_to_string(doc.join("README.md")).unwrap();
        assert!(readme.contains("](USER_GUIDE.md"), "README links the guide flat");
        assert!(!readme.contains("](docs/"), "README keeps a docs/ link");
        assert!(!prefix.join("lib/kiln/7/predefines/structure.diff").exists());
        assert!(!prefix.join("lib/kiln/7/include-paths").exists());
        #[cfg(unix)]
        for name in ["kiln++", "kiln-scan-deps", "clang-scan-deps-22", "clang-scan-deps"] {
            let link = prefix.join("bin").join(name);
            assert_eq!(std::fs::read_link(link).unwrap(), Path::new("kiln"), "{name}");
        }
        std::fs::write(prefix.join("bin/kiln"), "#!/bin/sh\n").unwrap();
        let (a, b) = (d.join("a.tar.xz"), d.join("b.tar.xz"));
        tar_xz(&d, "kiln-x", &a, 1_700_000_000).unwrap();
        // Touch the tree: the archive must not change.
        std::fs::write(prefix.join("bin/kiln"), "#!/bin/sh\n").unwrap();
        tar_xz(&d, "kiln-x", &b, 1_700_000_000).unwrap();
        assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        let list = Command::new("tar").arg("-tvJf").arg(&a).output().unwrap();
        let list = String::from_utf8_lossy(&list.stdout);
        assert!(list.lines().all(|l| l.contains(" 0/0 ")), "{list}");
        assert!(list.contains("kiln-x/bin/kiln++ -> kiln"), "{list}");
        let names: Vec<&str> = list.lines().filter_map(|l| l.split_whitespace().nth(5)).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn rewrites_links_for_the_flat_layout() {
        let root = util::repo_root();
        let t = "[g](docs/USER_GUIDE.md#c-mode) [s](spec/driver.md#x) [d](spec/) [a](#top) \
                 [u](https://x.org) <img src=\"assets/logo/kiln-banner.svg\">";
        let r = rewrite_links(&root, "README.md", t, "1.2.3").unwrap();
        let base = "https://github.com/christopherfretz/kiln";
        assert_eq!(
            r,
            format!(
                "[g](USER_GUIDE.md#c-mode) [s]({base}/blob/v1.2.3/spec/driver.md#x) \
                 [d]({base}/tree/v1.2.3/spec) [a](#top) [u](https://x.org) \
                 <img src=\"{base}/raw/v1.2.3/assets/logo/kiln-banner.svg\">"
            )
        );
        let r = rewrite_links(&root, "docs/USER_GUIDE.md", "[s](../SECURITY.md)", "1").unwrap();
        assert_eq!(r, "[s](SECURITY.md)");
        assert!(rewrite_links(&root, "README.md", "[m](docs/NOPE.md)", "1").is_err());
        assert!(rewrite_links(&root, "README.md", "[m](../x.md)", "1").is_err());
    }
}
