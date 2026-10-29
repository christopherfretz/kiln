//! The target, the language standard and the toolchain layout: where the
//! resource directory, the C++ standard library, GCC and the system headers
//! are, and the system part of the header search list clang 19's Linux
//! toolchain builds from them (`spec/driver.md` "Toolchain").

use std::path::{Path, PathBuf};

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
    /// (`KILN_LLVM_DIR`, default `/usr/lib/llvm-19`).
    pub llvm_dir: String,
    /// The GCC installation, if one was found.
    pub gcc: Option<GccInstall>,
}

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
            .unwrap_or_else(|| "/usr/lib/llvm-19".to_owned());
        let gcc = detect_gcc(&sysroot, &target);
        Toolchain { target, sysroot, resource_dir, data_dir, llvm_dir, gcc }
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

    /// clang `Linux::addLibCxxIncludePaths`: the first of
    /// `<llvm>/bin/../include`, `<sysroot>/usr/local/include`,
    /// `<sysroot>/usr/include` with a `c++/v1`, the per-target directory
    /// first if it exists.
    fn libcxx_dirs(&self, v: &mut Vec<SearchDirSpec>) {
        let bases = [
            format!("{}/bin/../include", self.llvm_dir),
            format!("{}/usr/local/include", self.sysroot),
            format!("{}/usr/include", self.sysroot),
        ];
        for base in bases {
            let generic = format!("{base}/c++/v1");
            if !Path::new(&generic).is_dir() {
                continue;
            }
            let per_target = format!("{base}/{}/c++/v1", self.target.triple);
            if Path::new(&per_target).is_dir() {
                v.push(SearchDirSpec::new(per_target, DirGroup::System));
            }
            v.push(SearchDirSpec::new(generic, DirGroup::System));
            return;
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
        if Path::new(&triple_dir).is_dir() {
            v.push(SearchDirSpec::new(triple_dir, DirGroup::System));
        } else if Path::new(&nested).is_dir() {
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
    /// `clang++`: the link step.
    Linker,
}

impl Tool {
    /// The versioned name on `PATH` (`llc-19`), as diagnostics name it.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Llc => "llc-19",
            Tool::LlvmAs => "llvm-as-19",
            Tool::Linker => "clang++-19",
        }
    }

    /// The name in an LLVM installation's `bin/` directory.
    fn llvm_bin_name(self) -> &'static str {
        match self {
            Tool::Llc => "llc",
            Tool::LlvmAs => "llvm-as",
            Tool::Linker => "clang++",
        }
    }

    /// The failure wording's stage: `<stage> command failed ...`.
    pub fn stage(self) -> &'static str {
        match self {
            Tool::Llc | Tool::LlvmAs => "backend",
            Tool::Linker => "linker",
        }
    }
}

impl Toolchain {
    /// Where `tool` is: `<llvm_dir>/bin/<tool>` (`KILN_LLVM_DIR`, default
    /// `/usr/lib/llvm-19`), else the versioned name (`llc-19`) on `PATH`.
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

/// The newest GCC under `<sysroot>/usr/lib/gcc/<triple>/<version>` (with a
/// `crtbegin.o`) for the target's candidate triples.
fn detect_gcc(sysroot: &str, target: &Target) -> Option<GccInstall> {
    let mut best: Option<(Vec<u32>, GccInstall)> = None;
    for triple in target.gcc_triples {
        let dir = format!("{sysroot}/usr/lib/gcc/{triple}");
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let version = e.file_name().to_string_lossy().into_owned();
            let Some(key) = gcc_version_key(&version) else { continue };
            let path = format!("{dir}/{version}");
            if !Path::new(&path).join("crtbegin.o").is_file() {
                continue;
            }
            if best.as_ref().is_none_or(|(k, _)| key > *k) {
                best = Some((key, GccInstall { path, triple: (*triple).to_owned(), version }));
            }
        }
    }
    best.map(|(_, g)| g)
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
}
