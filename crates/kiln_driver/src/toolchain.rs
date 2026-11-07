//! The target, the language standard and the toolchain layout: where the
//! resource directory, the C++ standard library, GCC and the system headers
//! are, and the system part of the header search list clang 19's Linux
//! toolchain builds from them (`spec/driver.md` "Toolchain").

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kiln_pp::{DirGroup, SearchDirSpec, join_path};

/// A supported target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    /// The normalized triple (`resource/predefines/<triple>.<std>.txt`).
    pub triple: &'static str,
    /// The architecture name (`__is_target_arch`).
    pub arch: &'static str,
    /// Debian's multiarch directory name (`/usr/include/<multiarch>`).
    pub multiarch: &'static str,
    /// GCC installation triples to look for, in clang's order.
    gcc_triples: &'static [&'static str],
}

const AARCH64: Target = Target {
    triple: "aarch64-unknown-linux-gnu",
    arch: "aarch64",
    multiarch: "aarch64-linux-gnu",
    gcc_triples: &[
        "aarch64-unknown-linux-gnu",
        "aarch64-linux-gnu",
        "aarch64-none-linux-gnu",
        "aarch64-redhat-linux",
        "aarch64-suse-linux",
    ],
};

const X86_64: Target = Target {
    triple: "x86_64-unknown-linux-gnu",
    arch: "x86_64",
    multiarch: "x86_64-linux-gnu",
    gcc_triples: &[
        "x86_64-unknown-linux-gnu",
        "x86_64-linux-gnu",
        "x86_64-pc-linux-gnu",
        "x86_64-redhat-linux",
        "x86_64-suse-linux",
        "x86_64-unknown-linux",
    ],
};

impl Target {
    /// The target of `--target=<s>`: an `aarch64`/`arm64` or
    /// `x86_64`/`amd64` Linux triple (vendor and environment may be
    /// omitted). `None` for anything else.
    pub fn parse(s: &str) -> Option<Target> {
        let mut parts = s.split('-');
        let t = match parts.next()? {
            "aarch64" | "arm64" => AARCH64,
            "x86_64" | "amd64" => X86_64,
            _ => return None,
        };
        let rest: Vec<&str> = parts.collect();
        // `<arch>`, `<arch>-<vendor>-linux[-<env>]` or `<arch>-linux[-<env>]`.
        let os_ok = rest.is_empty()
            || (rest.len() <= 3 && rest.iter().take(2).any(|p| p.starts_with("linux")));
        os_ok.then_some(t)
    }

    /// The host's target.
    pub fn host() -> Target {
        if cfg!(target_arch = "x86_64") { X86_64 } else { AARCH64 }
    }

    /// The target is `AArch64`.
    pub fn is_aarch64(&self) -> bool {
        self.arch == "aarch64"
    }
}

/// A language standard (`-std=`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LangStd {
    /// The standard's predefines name (`c++17`, `c11`, ...).
    pub base: &'static str,
    /// C++ (else C).
    pub cplusplus: bool,
    /// A GNU dialect (`gnu++17`, `gnu11`).
    pub gnu: bool,
    /// The lexer's standard. C is lexed as the closest C++ standard (see
    /// `spec/driver.md` "Deviations").
    pub lex: kiln_lex::Std,
}

impl LangStd {
    /// The standard named by `-std=<s>`.
    pub fn parse(s: &str) -> Option<LangStd> {
        use kiln_lex::Std as S;
        let (gnu, rest) = match s.strip_prefix("gnu") {
            Some(r) => (true, r),
            None => (false, s),
        };
        if let Some(v) = rest.strip_prefix("++") {
            if !gnu {
                return None;
            }
            return Self::cxx(v, true);
        }
        if !gnu && let Some(v) = s.strip_prefix("c++") {
            return Self::cxx(v, false);
        }
        let v = if gnu { rest } else { s };
        let (base, lex) = match v {
            "c89" | "c90" | "iso9899:1990" | "89" | "90" => ("c89", S::Cxx98),
            "c99" | "c9x" | "iso9899:1999" | "99" | "9x" => ("c99", S::Cxx98),
            "c11" | "c1x" | "iso9899:2011" | "11" | "1x" => ("c11", S::Cxx11),
            "c17" | "c18" | "iso9899:2017" | "iso9899:2018" | "17" | "18" => ("c17", S::Cxx11),
            "c23" | "c2x" | "iso9899:2024" | "23" | "2x" => ("c23", S::Cxx17),
            _ => return None,
        };
        // `gnu89` but not `gnuc89`; `c89` but not `89`.
        let spelled_ok = if gnu {
            !v.starts_with('c') && !v.starts_with("iso")
        } else {
            v.starts_with('c') || v.starts_with("iso")
        };
        spelled_ok.then_some(LangStd { base, cplusplus: false, gnu, lex })
    }

    fn cxx(v: &str, gnu: bool) -> Option<LangStd> {
        use kiln_lex::Std as S;
        let (base, lex) = match v {
            "98" => ("c++98", S::Cxx98),
            "03" => ("c++98", S::Cxx03),
            "11" | "0x" => ("c++11", S::Cxx11),
            "14" | "1y" => ("c++14", S::Cxx14),
            "17" | "1z" => ("c++17", S::Cxx17),
            "20" | "2a" => ("c++20", S::Cxx20),
            "23" | "2b" => ("c++23", S::Cxx23),
            "26" | "2c" => ("c++26", S::Cxx26),
            _ => return None,
        };
        Some(LangStd { base, cplusplus: true, gnu, lex })
    }

    /// clang's default: `gnu++17` for C++, `gnu17` for C.
    pub fn default_for(cplusplus: bool) -> LangStd {
        if cplusplus {
            LangStd { base: "c++17", cplusplus, gnu: true, lex: kiln_lex::Std::Cxx17 }
        } else {
            LangStd { base: "c17", cplusplus, gnu: true, lex: kiln_lex::Std::Cxx11 }
        }
    }

    /// C++20 or later.
    pub fn cxx20(&self) -> bool {
        self.cplusplus && self.lex >= kiln_lex::Std::Cxx20
    }
}

/// The C++ standard library (`-stdlib=`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StdLib {
    /// GNU libstdc++ (Debian's default).
    #[default]
    LibStdCxx,
    /// LLVM libc++.
    LibCxx,
}

/// A GCC installation (clang `GCCInstallationDetector`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GccInstall {
    /// `<prefix>/lib/gcc/<triple>/<version>`.
    pub path: String,
    /// The installation's triple (`aarch64-linux-gnu`).
    pub triple: String,
    /// The version directory name (`14`).
    pub version: String,
}

/// The header search options that select system directories.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemIncludes {
    /// `-nostdinc`: no system directories at all.
    pub nostdinc: bool,
    /// `-nostdinc++`: no C++ standard library directories.
    pub nostdincxx: bool,
    /// `-nostdlibinc`: no system or C++ directories (the resource
    /// directory stays).
    pub nostdlibinc: bool,
    /// `-nobuiltininc`: no resource directory.
    pub nobuiltininc: bool,
    /// The C++ standard library.
    pub stdlib: StdLib,
}

/// The toolchain: everything the driver locates on disk.
#[derive(Clone, Debug)]
pub struct Toolchain {
    /// The target.
    pub target: Target,
    /// `--sysroot` (empty for none).
    pub sysroot: String,
    /// The resource directory (`<dir>/include` holds the builtin headers).
    pub resource_dir: String,
    /// kiln's own data directory (`predefines/`), normally the same as the
    /// resource directory.
    pub data_dir: PathBuf,
    /// The LLVM installation whose libc++ headers are used
    /// (`KILN_LLVM_DIR`, default [`DEFAULT_LLVM_DIR`]).
    pub llvm_dir: String,
    /// The GCC installation, if one was found.
    pub gcc: Option<GccInstall>,
    /// The installations the search saw, in its order (`-v`'s `Found
    /// candidate GCC installation` lines; empty for `--gcc-install-dir`).
    pub gcc_candidates: Vec<String>,
    /// `gcc` is the installation [`DEFAULT_GCC`] reported, so the link
    /// step names it to `clang++` (`--gcc-install-dir`) too.
    pub gcc_from_driver: bool,
}

/// Where the GCC installation search looks (clang 19/22
/// `GCCInstallationDetector::init`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GccSearch<'a> {
    /// `<sysroot>/usr`.
    #[default]
    Default,
    /// `--gcc-toolchain=<dir>`: `<dir>` is the only prefix.
    Toolchain(&'a str),
    /// `--gcc-install-dir=<dir>`: `<dir>` is the installation
    /// (`.../<triple>/<version>`), if it has a `crtbegin.o`.
    InstallDir(&'a str),
}

/// The LLVM installation used when `KILN_LLVM_DIR` is unset or empty: the
/// `KILN_DEFAULT_LLVM_DIR` kiln was built with, if set and non-empty (a
/// distribution package whose LLVM lives elsewhere sets it), else
/// `/usr/lib/llvm-19`.
pub const DEFAULT_LLVM_DIR: &str = match option_env!("KILN_DEFAULT_LLVM_DIR") {
    Some(d) if !d.is_empty() => d,
    _ => "/usr/lib/llvm-19",
};

/// The GCC driver whose installation is used when there is no `--sysroot`,
/// `--gcc-toolchain` or `--gcc-install-dir`: the `KILN_DEFAULT_GCC` kiln
/// was built with, if set and non-empty (a distribution whose C++ compiler
/// is a GCC outside `/usr`, reached through a wrapper, sets it). The
/// installation is the `install:` line of its `-print-search-dirs`; when
/// that fails or names an installation for another target, the
/// `<sysroot>/usr` search runs as usual.
pub const DEFAULT_GCC: Option<&str> = match option_env!("KILN_DEFAULT_GCC") {
    Some(d) if !d.is_empty() => Some(d),
    _ => None,
};

/// kiln's default resource directory: `$KILN_RESOURCE_DIR`, else the
/// nearest `resource/` with a `predefines/` directory above the
/// executable, else the source tree's `resource/` (development builds).
pub fn default_resource_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("KILN_RESOURCE_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    if let Ok(exe) = std::env::current_exe() {
        for anc in exe.ancestors().skip(1) {
            let cand = anc.join("resource");
            if cand.join("predefines").is_dir() {
                return cand;
            }
        }
    }
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("resource"), |r| r.join("resource"))
}

impl Toolchain {
    /// Locates the toolchain for `target`.
    pub fn new(target: Target, sysroot: Option<&str>, resource_dir: Option<&str>) -> Toolchain {
        let default_res = default_resource_dir();
        let resource_dir =
            resource_dir.map_or_else(|| default_res.to_string_lossy().into_owned(), str::to_owned);
        let data_dir = if Path::new(&resource_dir).join("predefines").is_dir() {
            PathBuf::from(&resource_dir)
        } else {
            default_res
        };
        let sysroot = sysroot.unwrap_or("").to_owned();
        let llvm_dir = std::env::var("KILN_LLVM_DIR")
            .ok()
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| DEFAULT_LLVM_DIR.to_owned());
        let driver_gcc = DEFAULT_GCC
            .filter(|_| sysroot.is_empty())
            .and_then(driver_gcc_install)
            .filter(|g| target.gcc_triples.contains(&g.triple.as_str()));
        let gcc_from_driver = driver_gcc.is_some();
        let (gcc, gcc_candidates) = match driver_gcc {
            Some(g) => (Some(g), Vec::new()),
            None => detect_gcc(&format!("{sysroot}/usr"), &target),
        };
        Toolchain {
            target,
            sysroot,
            resource_dir,
            data_dir,
            llvm_dir,
            gcc,
            gcc_candidates,
            gcc_from_driver,
        }
    }

    /// Redoes the GCC installation search for `--gcc-toolchain=` or
    /// `--gcc-install-dir=` (the latter wins, as in clang). `Err(dir)` when
    /// the install directory holds no GCC installation (clang
    /// `err_drv_invalid_gcc_install_dir`); there is then no GCC.
    pub fn select_gcc(&mut self, search: GccSearch<'_>) -> Result<(), String> {
        self.gcc_from_driver = false;
        match search {
            GccSearch::Default => {
                (self.gcc, self.gcc_candidates) =
                    detect_gcc(&format!("{}/usr", self.sysroot), &self.target);
            }
            GccSearch::Toolchain(dir) => {
                let prefix = dir.strip_suffix('/').unwrap_or(dir);
                (self.gcc, self.gcc_candidates) = detect_gcc(prefix, &self.target);
            }
            GccSearch::InstallDir(dir) => {
                self.gcc_candidates.clear();
                self.gcc = gcc_install_dir(dir);
                if self.gcc.is_none() {
                    return Err(dir.to_owned());
                }
            }
        }
        Ok(())
    }

    /// The predefines base file for `std`.
    pub fn predefines_path(&self, std: &LangStd) -> PathBuf {
        self.data_dir.join("predefines").join(format!("{}.{}.txt", self.target.triple, std.base))
    }

    /// The toolchain's include directories, after the user's, in clang's
    /// order (`Linux::AddClangCXXStdlibIncludeArgs`, then
    /// `Linux::AddClangSystemIncludeArgs`).
    pub fn system_include_specs(&self, o: &SystemIncludes, cplusplus: bool) -> Vec<SearchDirSpec> {
        let mut v = Vec::new();
        if o.nostdinc {
            return v;
        }
        let sys =
            |v: &mut Vec<SearchDirSpec>, p: String| v.push(SearchDirSpec::new(p, DirGroup::System));
        if cplusplus && !o.nostdincxx && !o.nostdlibinc {
            match o.stdlib {
                StdLib::LibCxx => self.libcxx_dirs(&mut v),
                StdLib::LibStdCxx => self.libstdcxx_dirs(&mut v),
            }
        }
        if !o.nobuiltininc {
            sys(&mut v, join_path(&self.resource_dir, "include"));
        }
        if o.nostdlibinc {
            return v;
        }
        let root = &self.sysroot;
        sys(&mut v, format!("{root}/usr/local/include"));
        if let Some(g) = &self.gcc {
            sys(&mut v, format!("{}/../../../../{}/include", g.path, g.triple));
        }
        let externc = |v: &mut Vec<SearchDirSpec>, p: String| {
            v.push(SearchDirSpec::new(p, DirGroup::ExternCSystem));
        };
        let multiarch = format!("{root}/usr/include/{}", self.target.multiarch);
        if Path::new(&multiarch).is_dir() {
            externc(&mut v, multiarch);
        }
        externc(&mut v, format!("{root}/include"));
        externc(&mut v, format!("{root}/usr/include"));
        v
    }

    /// Whether the C++ standard library `stdlib` ships a `<contracts>`
    /// header, and with it its own default contract-violation handler
    /// (libstdc++ from GCC 16; spec/contracts.md §6.3). Otherwise kiln's
    /// resource-directory `<contracts>` is the one found, and codegen
    /// emits the default handler.
    pub fn stdlib_ships_contracts(&self, stdlib: StdLib) -> bool {
        let mut v = Vec::new();
        match stdlib {
            StdLib::LibCxx => self.libcxx_dirs(&mut v),
            StdLib::LibStdCxx => self.libstdcxx_dirs(&mut v),
        }
        v.iter().any(|d| Path::new(&d.path).join("contracts").is_file())
    }

    /// clang `Linux::addLibCxxIncludePaths`: the first of
    /// `<llvm>/bin/../include`, `<sysroot>/usr/local/include`,
    /// `<sysroot>/usr/include` with a `c++/v1`, the per-target directory
    /// first if it exists.
    fn libcxx_dirs(&self, v: &mut Vec<SearchDirSpec>) {
        let Some(base) = self.libcxx_include_base() else { return };
        let per_target = format!("{base}/{}/c++/v1", self.target.triple);
        if Path::new(&per_target).is_dir() {
            v.push(SearchDirSpec::new(per_target, DirGroup::System));
        }
        v.push(SearchDirSpec::new(format!("{base}/c++/v1"), DirGroup::System));
    }

    /// The include base holding the libc++ headers (`<base>/c++/v1`):
    /// the first of `<llvm>/bin/../include`, `<sysroot>/usr/local/include`,
    /// `<sysroot>/usr/include` that has one, as [`Self::libcxx_dirs`].
    fn libcxx_include_base(&self) -> Option<String> {
        [
            format!("{}/bin/../include", self.llvm_dir),
            format!("{}/usr/local/include", self.sysroot),
            format!("{}/usr/include", self.sysroot),
        ]
        .into_iter()
        .find(|base| Path::new(&format!("{base}/c++/v1")).is_dir())
    }

    /// Where libc++'s module interface sources (`std.cppm`,
    /// `std.compat.cppm`) are (spec/modules.md §2): `<prefix>/share/libc++/v1`
    /// for the `<prefix>/include/c++/v1` that `-stdlib=libc++` uses, with
    /// symlinks resolved (Debian's `/usr/include/c++/v1` points into
    /// `/usr/lib/llvm-N`), else `<llvm>/share/libc++/v1`.
    pub fn libcxx_module_dir(&self) -> PathBuf {
        let fallback = Path::new(&self.llvm_dir).join("share").join("libc++").join("v1");
        let Some(base) = self.libcxx_include_base() else { return fallback };
        let v1 = Path::new(&base).join("c++").join("v1");
        let v1 = std::fs::canonicalize(&v1).unwrap_or(v1);
        // `<prefix>/include/c++/v1` -> `<prefix>/share/libc++/v1`.
        match v1.parent().and_then(Path::parent).and_then(Path::parent) {
            Some(prefix) => {
                let share = prefix.join("share").join("libc++").join("v1");
                if share.is_dir() { share } else { fallback }
            }
            None => fallback,
        }
    }

    /// clang `Generic_GCC::addGCCLibStdCxxIncludePaths`.
    fn libstdcxx_dirs(&self, v: &mut Vec<SearchDirSpec>) {
        let Some(g) = &self.gcc else { return };
        let inc = format!("{}/../../../../include", g.path);
        let base = format!("{inc}/c++/{}", g.version);
        if !Path::new(&base).is_dir() {
            return;
        }
        v.push(SearchDirSpec::new(base.clone(), DirGroup::System));
        let triple_dir = format!("{inc}/{}/c++/{}", g.triple, g.version);
        let nested = format!("{base}/{}", g.triple);
        // clang `addLibStdCXXIncludePaths`: Debian's multiarch directory if
        // it exists, else the nested one (listed even when missing, so `-v`
        // reports it as nonexistent).
        if Path::new(&triple_dir).is_dir() {
            v.push(SearchDirSpec::new(triple_dir, DirGroup::System));
        } else {
            v.push(SearchDirSpec::new(nested, DirGroup::System));
        }
        v.push(SearchDirSpec::new(format!("{base}/backward"), DirGroup::System));
    }
}

/// An external LLVM tool the driver runs (`spec/codegen.md` §5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// `llc`: IR to assembly (`-S`) or an object file (`-c`, linking).
    Llc,
    /// `llvm-as`: IR to bitcode (`-c -emit-llvm`).
    LlvmAs,
    /// `opt`: the coroutine lowering before `llc` (only for a module
    /// that declares an `llvm.coro.*` intrinsic).
    Opt,
    /// `clang++`: the link step.
    Linker,
}

impl Tool {
    /// The versioned name on `PATH` (`llc-19`), as diagnostics name it.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Llc => "llc-19",
            Tool::LlvmAs => "llvm-as-19",
            Tool::Opt => "opt-19",
            Tool::Linker => "clang++-19",
        }
    }

    /// The name in an LLVM installation's `bin/` directory.
    fn llvm_bin_name(self) -> &'static str {
        match self {
            Tool::Llc => "llc",
            Tool::LlvmAs => "llvm-as",
            Tool::Opt => "opt",
            Tool::Linker => "clang++",
        }
    }

    /// The failure wording's stage: `<stage> command failed ...`.
    pub fn stage(self) -> &'static str {
        match self {
            Tool::Llc | Tool::LlvmAs | Tool::Opt => "backend",
            Tool::Linker => "linker",
        }
    }
}

impl Toolchain {
    /// The major version of the LLVM installation (`KILN_LLVM_DIR`) that
    /// consumes kiln's IR: the major of `<llvm_dir>/bin/llvm-config
    /// --version` (probed once per directory per process), else the `N` of
    /// a directory named `llvm-N` (after resolving symlinks), else 19, the
    /// version of the unversioned default and of the `-19` tool names
    /// (spec/driver.md "Toolchain").
    pub fn llvm_major(&self) -> u32 {
        let dir = Path::new(&self.llvm_dir);
        probe_llvm_config_major(dir)
            .or_else(|| llvm_dir_major(dir))
            .or_else(|| std::fs::canonicalize(dir).ok().and_then(|d| llvm_dir_major(&d)))
            .unwrap_or(19)
    }

    /// Where `tool` is: `<llvm_dir>/bin/<tool>` (`KILN_LLVM_DIR`, default
    /// [`DEFAULT_LLVM_DIR`]), else the versioned name (`llc-19`) on `PATH`.
    /// `None` if neither exists.
    pub fn find_tool(&self, tool: Tool) -> Option<PathBuf> {
        let in_llvm = Path::new(&self.llvm_dir).join("bin").join(tool.llvm_bin_name());
        if in_llvm.is_file() {
            return Some(in_llvm);
        }
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path).map(|d| d.join(tool.name())).find(|p| p.is_file())
    }
}

/// The newest GCC under `<prefix>/{lib64,lib}/gcc/<triple>/<version>`
/// (with a `crtbegin.o`) for the target's candidate triples, ties going to
/// the first found, and every version directory seen, sorted (clang's
/// `ScanLibDirForGCCTriple` over `CandidateLibDirs`; its
/// `CandidateGCCInstallPaths` is a sorted set filled before the
/// `crtbegin.o` check).
fn detect_gcc(prefix: &str, target: &Target) -> (Option<GccInstall>, Vec<String>) {
    let mut best: Option<(Vec<u32>, GccInstall)> = None;
    let mut seen = std::collections::BTreeSet::new();
    for lib in ["lib64", "lib"] {
        for triple in target.gcc_triples {
            let dir = format!("{prefix}/{lib}/gcc/{triple}");
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let version = e.file_name().to_string_lossy().into_owned();
                let Some(key) = gcc_version_key(&version) else { continue };
                let path = format!("{dir}/{version}");
                seen.insert(path.clone());
                if key < vec![4, 1, 1] || !Path::new(&path).join("crtbegin.o").is_file() {
                    continue;
                }
                if best.as_ref().is_none_or(|(k, _)| key > *k) {
                    best = Some((key, GccInstall { path, triple: (*triple).to_owned(), version }));
                }
            }
        }
    }
    (best.map(|(_, g)| g), seen.into_iter().collect())
}

/// The installation the GCC driver `gcc` reports: the `install:` line of
/// `gcc -print-search-dirs`, if it names an installation (see
/// [`gcc_install_dir`]). `None` when the driver is missing or fails.
fn driver_gcc_install(gcc: &str) -> Option<GccInstall> {
    let out = std::process::Command::new(gcc)
        .arg("-print-search-dirs")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let dir = text.lines().find_map(|l| l.strip_prefix("install:"))?.trim();
    gcc_install_dir(dir)
}

/// The installation `--gcc-install-dir=<dir>` names: `<dir>` (without a
/// trailing `/`) if it has a `crtbegin.o`; its triple and version are the
/// names of its parent and of itself.
fn gcc_install_dir(dir: &str) -> Option<GccInstall> {
    let path = dir.strip_suffix('/').unwrap_or(dir);
    if !Path::new(path).join("crtbegin.o").is_file() {
        return None;
    }
    let p = Path::new(path);
    let version = p.file_name()?.to_string_lossy().into_owned();
    let triple = p.parent()?.file_name()?.to_string_lossy().into_owned();
    Some(GccInstall { path: path.to_owned(), triple, version })
}

/// The major of `<dir>/bin/llvm-config --version`, cached per directory
/// for the life of the process (the driver asks once per compile job; a
/// process normally sees one installation). `None` when the tool is
/// missing, fails, or prints something that is not `N.M.P...`.
fn probe_llvm_config_major(dir: &Path) -> Option<u32> {
    static CACHE: Mutex<Vec<(PathBuf, Option<u32>)>> = Mutex::new(Vec::new());
    let mut cache = CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, m)) = cache.iter().find(|(d, _)| d == dir) {
        return *m;
    }
    let tool = dir.join("bin").join("llvm-config");
    let m = if tool.is_file() {
        std::process::Command::new(&tool)
            .arg("--version")
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| llvm_config_version_major(&String::from_utf8_lossy(&o.stdout)))
    } else {
        None
    };
    cache.push((dir.to_path_buf(), m));
    m
}

/// `22.1.8`, `19.1.7git` -> 22, 19: the leading integer of
/// `llvm-config --version`'s output.
fn llvm_config_version_major(out: &str) -> Option<u32> {
    let major = out.trim().split('.').next()?;
    if major.is_empty() || !major.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    major.parse().ok()
}

/// `/usr/lib/llvm-22` -> `22`; `None` unless the last component is
/// `llvm-N`.
fn llvm_dir_major(dir: &Path) -> Option<u32> {
    dir.file_name()?.to_str()?.strip_prefix("llvm-")?.parse().ok()
}

/// `14`, `13.2.0` -> `[14]`, `[13, 2, 0]`; `None` if not a version.
fn gcc_version_key(s: &str) -> Option<Vec<u32>> {
    s.split('.').map(|p| p.parse::<u32>().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets() {
        assert_eq!(Target::parse("aarch64-unknown-linux-gnu"), Some(AARCH64));
        assert_eq!(Target::parse("aarch64-linux-gnu"), Some(AARCH64));
        assert_eq!(Target::parse("x86_64-pc-linux-gnu"), Some(X86_64));
        assert_eq!(Target::parse("x86_64"), Some(X86_64));
        assert_eq!(Target::parse("x86_64-apple-darwin"), None);
        assert_eq!(Target::parse("riscv64-unknown-linux-gnu"), None);
    }

    #[test]
    fn stds() {
        let s = LangStd::parse("gnu++2c").unwrap();
        assert!(s.cplusplus && s.gnu && s.base == "c++26");
        let s = LangStd::parse("c++03").unwrap();
        assert_eq!((s.base, s.lex), ("c++98", kiln_lex::Std::Cxx03));
        let s = LangStd::parse("c11").unwrap();
        assert!(!s.cplusplus && !s.gnu && s.base == "c11");
        let s = LangStd::parse("gnu99").unwrap();
        assert!(s.gnu && s.base == "c99");
        assert_eq!(LangStd::parse("iso9899:2017").map(|s| s.base), Some("c17"));
        for bad in ["c++27", "gnu", "c", "gnuc11", "99", "gnu++", "c++"] {
            assert_eq!(LangStd::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn llvm_dir_majors() {
        assert_eq!(llvm_dir_major(Path::new("/usr/lib/llvm-22")), Some(22));
        assert_eq!(llvm_dir_major(Path::new("/usr/lib/llvm-19/")), Some(19));
        assert_eq!(llvm_dir_major(Path::new("/opt/llvm")), None);
        assert_eq!(llvm_dir_major(Path::new("/opt/llvm-main")), None);
    }

    #[test]
    fn llvm_config_versions() {
        assert_eq!(llvm_config_version_major("22.1.8\n"), Some(22));
        assert_eq!(llvm_config_version_major("19.1.7"), Some(19));
        assert_eq!(llvm_config_version_major("23.0.0git\n"), Some(23));
        assert_eq!(llvm_config_version_major(""), None);
        assert_eq!(llvm_config_version_major("x.1"), None);
    }

    /// The probe keys on the installed tool, not the directory name; a
    /// directory without one falls back to the name, then 19.
    #[test]
    fn llvm_major_probe() {
        let tc =
            |d: &str| Toolchain { llvm_dir: d.to_owned(), ..Toolchain::new(AARCH64, None, None) };
        assert_eq!(tc("/nonexistent/llvm-22").llvm_major(), 22);
        assert_eq!(tc("/nonexistent/llvm").llvm_major(), 19);
        for n in [19, 22] {
            let d = format!("/usr/lib/llvm-{n}");
            if Path::new(&d).join("bin/llvm-config").is_file() {
                assert_eq!(tc(&d).llvm_major(), n);
            }
        }
    }

    #[test]
    fn gcc_versions() {
        assert_eq!(gcc_version_key("13.2.0"), Some(vec![13, 2, 0]));
        assert_eq!(gcc_version_key("14"), Some(vec![14]));
        assert_eq!(gcc_version_key("x"), None);
    }
}
