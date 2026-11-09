//! The target, the language standard and the toolchain layout: where the
//! resource directory, the C++ standard library, GCC and the system headers
//! are, and the system part of the header search list clang 19's Linux
//! toolchain builds from them (`spec/driver.md` "Toolchain").

use std::path::{Path, PathBuf};

use kiln_pp::{DirGroup, SearchDirSpec, join_path};

use crate::llvm::{DEFAULT_LLVM_MAJOR, LlvmSelection, Search};

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
    /// The LLVM installation whose tools run and whose libc++ headers are
    /// used, or why there is none ([`crate::llvm`]).
    pub llvm: LlvmSelection,
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

/// The resource directory below an installation prefix (spec/driver.md
/// "Installation layout"): `<prefix>/lib/kiln/<major>`, beside
/// `<prefix>/bin/kiln`, as clang's `lib/clang/<major>`.
pub const INSTALL_RESOURCE_SUBDIR: [&str; 2] = ["lib", "kiln"];

/// The installed resource directory for the executable `exe` (symlinks
/// already resolved): `<exe dir>/../lib/kiln/<major>`.
pub fn install_resource_dir(exe: &Path) -> Option<PathBuf> {
    let prefix = exe.parent()?.parent()?;
    let mut d = prefix.to_path_buf();
    d.extend(INSTALL_RESOURCE_SUBDIR);
    d.push(crate::version::MAJOR.to_string());
    Some(d)
}

/// kiln's default resource directory (spec/driver.md "Toolchain"), the
/// first of:
///
/// 1. `$KILN_RESOURCE_DIR` (non-empty);
/// 2. the installed resource directory of the executable, resolved
///    through symlinks ([`install_resource_dir`]), if it has a
///    `predefines/` directory;
/// 3. the nearest `resource/` with a `predefines/` directory above the
///    executable (a checkout's `target/<profile>/kiln`);
/// 4. in a development build, the source tree's `resource/`; in a `dist`
///    build (feature `dist`, `cargo xtask dist`), which must never name
///    the build host's checkout, the installed path of 2 even though it is
///    missing, so the error names where the files belong.
pub fn default_resource_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("KILN_RESOURCE_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    let exe = std::env::current_exe().ok().map(|e| std::fs::canonicalize(&e).unwrap_or(e));
    let installed = exe.as_deref().and_then(install_resource_dir);
    if let Some(d) = &installed
        && d.join("predefines").is_dir()
    {
        return d.clone();
    }
    if let Some(exe) = &exe {
        for anc in exe.ancestors().skip(1) {
            let cand = anc.join("resource");
            if cand.join("predefines").is_dir() {
                return cand;
            }
        }
    }
    fallback_resource_dir(installed)
}

/// Step 4 of [`default_resource_dir`] in a development build: the source
/// tree's `resource/`.
#[cfg(not(feature = "dist"))]
fn fallback_resource_dir(_installed: Option<PathBuf>) -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("resource"), |r| r.join("resource"))
}

/// Step 4 of [`default_resource_dir`] in a `dist` build: the installed
/// path, which does not exist (no build-host path is compiled in).
#[cfg(feature = "dist")]
fn fallback_resource_dir(installed: Option<PathBuf>) -> PathBuf {
    installed.unwrap_or_else(|| {
        let mut d: PathBuf = INSTALL_RESOURCE_SUBDIR.iter().collect();
        d.push(crate::version::MAJOR.to_string());
        d
    })
}

impl Toolchain {
    /// Locates the toolchain for `target`; `llvm_dir` is `--llvm-dir=`
    /// (spec/driver.md "LLVM discovery").
    pub fn new(
        target: Target,
        sysroot: Option<&str>,
        resource_dir: Option<&str>,
        llvm_dir: Option<&str>,
    ) -> Toolchain {
        let default_res = default_resource_dir();
        let resource_dir =
            resource_dir.map_or_else(|| default_res.to_string_lossy().into_owned(), str::to_owned);
        let data_dir = if Path::new(&resource_dir).join("predefines").is_dir() {
            PathBuf::from(&resource_dir)
        } else {
            default_res
        };
        let sysroot = sysroot.unwrap_or("").to_owned();
        let llvm = Search::from_env(llvm_dir).run();
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
            llvm,
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

    /// Whether `-moutline-atomics` is the default on `AArch64` (clang 19/22
    /// `Linux::IsAArch64OutlineAtomicsDefault`, the libgcc runtime): only
    /// with a GCC installation of 9.3.1 or later, whose libgcc has the
    /// helpers. Without one (an `AArch64` cross compile with no `AArch64`
    /// GCC found) the atomics are inline.
    pub fn aarch64_outline_atomics_default(&self) -> bool {
        self.gcc.as_ref().is_some_and(|g| !gcc_version_older_than(&g.version, (9, 3, 1)))
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
        let llvm = self.llvm_dir().map(|d| format!("{}/bin/../include", d.display()));
        llvm.into_iter()
            .chain([
                format!("{}/usr/local/include", self.sysroot),
                format!("{}/usr/include", self.sysroot),
            ])
            .find(|base| Path::new(&format!("{base}/c++/v1")).is_dir())
    }

    /// Where libc++'s module interface sources (`std.cppm`,
    /// `std.compat.cppm`) are (spec/modules.md §2): `<prefix>/share/libc++/v1`
    /// for the `<prefix>/include/c++/v1` that `-stdlib=libc++` uses, with
    /// symlinks resolved (Debian's `/usr/include/c++/v1` points into
    /// `/usr/lib/llvm-N`), else `<llvm>/share/libc++/v1`.
    pub fn libcxx_module_dir(&self) -> PathBuf {
        let fallback = self
            .llvm_dir()
            .unwrap_or_else(|| Path::new("/usr"))
            .join("share")
            .join("libc++")
            .join("v1");
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

    /// libc++'s module manifest, `libc++.modules.json` (what
    /// `-print-file-name=libc++.modules.json` prints, and what `cmake`
    /// reads for `CXX_MODULE_STD`): beside the libc++ whose module sources
    /// [`Self::libcxx_module_dir`] names, so its relative `source-path`
    /// entries resolve to those sources: `<prefix>/lib/<triple>/` or
    /// `<prefix>/lib/`, else the LLVM directory's `lib/`.
    pub fn libcxx_modules_json(&self) -> Option<PathBuf> {
        const NAME: &str = "libc++.modules.json";
        let dir = self.libcxx_module_dir();
        // `<prefix>/share/libc++/v1` -> `<prefix>`.
        let prefix = dir.parent().and_then(Path::parent).and_then(Path::parent);
        let mut candidates = Vec::new();
        if let Some(prefix) = prefix {
            candidates.push(prefix.join("lib").join(self.target.triple).join(NAME));
            candidates.push(prefix.join("lib").join(NAME));
        }
        if let Some(llvm) = self.llvm_dir() {
            candidates.push(llvm.join("lib").join(NAME));
        }
        candidates.into_iter().find(|p| p.is_file())
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
    /// `llvm-objcopy`: `-gz` compresses an object's debug sections.
    Objcopy,
}

impl Tool {
    /// The name in an LLVM installation's `bin/` directory, as
    /// diagnostics and `-###` name a tool that was not found.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Llc => "llc",
            Tool::LlvmAs => "llvm-as",
            Tool::Opt => "opt",
            Tool::Linker => "clang++",
            Tool::Objcopy => "llvm-objcopy",
        }
    }

    /// The failure wording's stage: `<stage> command failed ...`.
    pub fn stage(self) -> &'static str {
        match self {
            Tool::Llc | Tool::LlvmAs | Tool::Opt | Tool::Objcopy => "backend",
            Tool::Linker => "linker",
        }
    }
}

impl Toolchain {
    /// The selected LLVM installation's prefix, if one was found.
    pub fn llvm_dir(&self) -> Option<&Path> {
        self.llvm.as_ref().ok().map(|i| i.dir.as_path())
    }

    /// The major version of the LLVM installation that consumes kiln's IR
    /// ([`crate::llvm::dir_major`]), else [`DEFAULT_LLVM_MAJOR`] when none
    /// was found (`-S -emit-llvm` runs no tool; spec/driver.md "LLVM
    /// discovery").
    pub fn llvm_major(&self) -> u32 {
        self.llvm.as_ref().map_or(DEFAULT_LLVM_MAJOR, |i| i.major)
    }

    /// Where `tool` is: `<llvm>/bin/<tool>` of the selected installation.
    /// `None` without one, or if it has no such file.
    pub fn find_tool(&self, tool: Tool) -> Option<PathBuf> {
        let p = self.llvm_dir()?.join("bin").join(tool.name());
        p.is_file().then_some(p)
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

/// clang `GCCVersion::isOlderThan` on the parsed version directory name
/// `v` (`GCCVersion::Parse`: up to three `.`-separated numbers, a
/// missing minor or patch is "any", so `9` is not older than 9.3.1; an
/// unparsable major is older than everything).
fn gcc_version_older_than(v: &str, (major, minor, patch): (i32, i32, i32)) -> bool {
    let num = |s: Option<&str>| -> i32 {
        let s = s.unwrap_or("");
        let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        s[..end].parse().unwrap_or(-1)
    };
    let mut it = v.splitn(3, '.');
    let (v_major, v_minor, v_patch) = (num(it.next()), num(it.next()), num(it.next()));
    if v_major != major {
        return v_major < major;
    }
    if v_minor != minor {
        return v_minor != -1 && v_minor < minor;
    }
    v_patch != patch && v_patch != -1 && v_patch < patch
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
    fn gcc_versions() {
        assert_eq!(gcc_version_key("13.2.0"), Some(vec![13, 2, 0]));
        assert_eq!(gcc_version_key("14"), Some(vec![14]));
        assert_eq!(gcc_version_key("x"), None);
    }

    // clang `GCCVersion::isOlderThan` against 9.3.1 (WP-808).
    #[test]
    fn gcc_version_older_than_clang() {
        let old = |v| gcc_version_older_than(v, (9, 3, 1));
        assert!(old("8") && old("9.2") && old("9.3.0") && old("x"));
        assert!(!old("9") && !old("9.3") && !old("9.3.1") && !old("9.4.0") && !old("14"));
        assert!(!old("9.3.x") && !old("10-win32") && !old("16.1.0"));
    }
}
