//! The coverage instrumentation flags (WP-941, spec/driver.md "Coverage
//! flags"), as clang 19's driver reads them (`addPGOAndCoverageFlags`):
//! gcov (`--coverage`, `-fprofile-arcs`, `-ftest-coverage`) and
//! source-based coverage (`-fprofile-instr-generate[=<path>]`,
//! `-fcoverage-mapping`). Profile-guided optimization (`-fprofile-use`,
//! `-fprofile-instr-use`, and `-fprofile-generate`, which clang turns into
//! IR-level PGO instrumentation rather than coverage) is a known kiln 1.0
//! limitation and is rejected by name.

use std::path::Path;

use crate::args::ArgError;

/// The coverage flags of a command line.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoverageFlags {
    /// `--coverage` was given (anywhere).
    pub coverage: bool,
    /// The last of `-fprofile-arcs` / `-fno-profile-arcs`.
    pub profile_arcs: Option<bool>,
    /// The last of `-ftest-coverage` / `-fno-test-coverage`.
    pub test_coverage: Option<bool>,
    /// The last of `-fprofile-instr-generate[=<path>]` /
    /// `-fno-profile-instr-generate`: `Some(path)` when on (`path` empty
    /// for the runtime's default `default.profraw`).
    pub instr_generate: Option<Option<String>>,
    /// The last of `-fcoverage-mapping` / `-fno-coverage-mapping`.
    pub mapping: Option<bool>,
    /// The last `-fprofile-dir=`.
    pub profile_dir: Option<String>,
    /// `-fprofile-update=atomic|prefer-atomic` was last (`single` clears).
    pub atomic: bool,
    /// The last `-fcoverage-compilation-dir=` / `-ffile-compilation-dir=`.
    pub comp_dir: Option<String>,
    /// `-fcoverage-prefix-map=` and `-ffile-prefix-map=` entries, in
    /// order.
    pub prefix_map: Vec<(String, String)>,
    /// The last `-dumpdir`.
    pub dumpdir: Option<String>,
    /// The last `-fprofile-filter-files=` / `-fprofile-exclude-files=`.
    pub filter: Option<String>,
    /// `-fprofile-generate` (IR PGO) was given and not cancelled.
    pub pgo_generate: bool,
}

/// The profile-guided optimization flags kiln 1.0 does not implement,
/// exactly as written or as a `=`-valued prefix.
const PGO: &[&str] = &[
    "-fprofile-use",
    "-fprofile-instr-use",
    "-fprofile-sample-use",
    "-fauto-profile",
    "-fcs-profile-generate",
    "-fprofile-remapping-file",
];

/// The message of a rejected PGO flag (shown as "'<it>' is not supported
/// by kiln").
fn pgo_error(arg: &str) -> ArgError {
    let what = if arg.starts_with("-fprofile-generate") || arg.starts_with("-fcs-profile-generate")
    {
        "IR profile-guided optimization instrumentation"
    } else {
        "profile-guided optimization"
    };
    ArgError::UnsupportedByKiln(format!(
        "{arg} ({what} is a known kiln 1.0 limitation; for coverage use \
         --coverage or -fprofile-instr-generate)"
    ))
}

impl CoverageFlags {
    /// Takes `arg` if it is one of the flags: `None` if it is not (or if
    /// another table also reads it: `-ffile-compilation-dir=`,
    /// `-ffile-prefix-map=`), else whether it is accepted.
    pub fn parse_arg(&mut self, arg: &str) -> Option<Result<(), ArgError>> {
        match arg {
            "--coverage" | "-coverage" => self.coverage = true,
            "-fprofile-arcs" => self.profile_arcs = Some(true),
            "-fno-profile-arcs" => self.profile_arcs = Some(false),
            "-ftest-coverage" => self.test_coverage = Some(true),
            "-fno-test-coverage" => self.test_coverage = Some(false),
            "-fprofile-instr-generate" => self.instr_generate = Some(Some(String::new())),
            "-fno-profile-instr-generate" => self.instr_generate = None,
            "-fcoverage-mapping" => self.mapping = Some(true),
            "-fno-coverage-mapping" => self.mapping = Some(false),
            "-fprofile-generate" => self.pgo_generate = true,
            "-fno-profile-generate" => self.pgo_generate = false,
            _ if PGO.contains(&arg) => return Some(Err(pgo_error(arg))),
            _ => return self.valued(arg),
        }
        Some(Ok(()))
    }

    fn valued(&mut self, arg: &str) -> Option<Result<(), ArgError>> {
        let (flag, v) = arg.split_once('=')?;
        match flag {
            "-fprofile-instr-generate" => self.instr_generate = Some(Some(v.to_owned())),
            "-fprofile-generate" => self.pgo_generate = true,
            _ if PGO.contains(&flag) => return Some(Err(pgo_error(arg))),
            "-fprofile-dir" => self.profile_dir = Some(v.to_owned()),
            "-fprofile-update" => match v {
                "atomic" | "prefer-atomic" => self.atomic = true,
                "single" => self.atomic = false,
                _ => {
                    return Some(Err(ArgError::UnsupportedValue(
                        "-fprofile-update=".into(),
                        v.to_owned(),
                    )));
                }
            },
            "-fcoverage-compilation-dir" => self.comp_dir = Some(v.to_owned()),
            "-ffile-compilation-dir" => {
                self.comp_dir = Some(v.to_owned());
                return None;
            }
            "-fcoverage-prefix-map" | "-ffile-prefix-map" => {
                if let Some((old, new)) = v.split_once('=') {
                    self.prefix_map.push((old.to_owned(), new.to_owned()));
                } else if flag == "-fcoverage-prefix-map" {
                    return Some(Err(ArgError::InvalidArgumentToOption(
                        v.to_owned(),
                        "fcoverage-prefix-map=".to_owned(),
                    )));
                }
                if flag == "-ffile-prefix-map" {
                    return None;
                }
            }
            "-fprofile-filter-files" | "-fprofile-exclude-files" => {
                self.filter = Some(format!("{flag}="));
            }
            _ => return None,
        }
        Some(Ok(()))
    }

    /// clang `EmitCovNotes`: a `.gcno` notes file.
    pub fn gcov_notes(&self) -> bool {
        self.test_coverage == Some(true) || self.coverage
    }

    /// clang `needsGCovInstrumentation`: `.gcda` counters.
    pub fn gcov_data(&self) -> bool {
        self.profile_arcs == Some(true) || self.coverage
    }

    /// Whether gcov instrumentation is on (notes, counters, or both).
    pub fn gcov(&self) -> bool {
        self.gcov_notes() || self.gcov_data()
    }

    /// Whether source-based coverage mapping is on.
    pub fn mapping(&self) -> bool {
        self.instr_generate.is_some() && self.mapping == Some(true)
    }

    /// Whether the linking `clang++` needs the profile runtime: the flag
    /// it is given (`--coverage` covers the gcov runtime, which is the same
    /// `libclang_rt.profile`; `-fprofile-instr-generate` also forces
    /// `__llvm_profile_runtime`).
    pub fn link_flags(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.gcov_data() || self.gcov_notes() {
            v.push("--coverage");
        }
        if self.instr_generate.is_some() {
            v.push("-fprofile-instr-generate");
        }
        v
    }

    /// The command-line errors clang's driver reports for the
    /// combination, and the ones kiln adds (spec/driver.md "Coverage
    /// flags").
    pub fn errors(&self) -> Vec<ArgError> {
        let mut e = Vec::new();
        if self.pgo_generate {
            e.push(pgo_error("-fprofile-generate"));
        }
        // The counters land without their mapping (spec/driver.md
        // "Coverage flags"): an explicit error rather than a binary that
        // `llvm-cov` cannot read.
        if self.mapping() {
            e.push(ArgError::UnsupportedByKiln(
                "-fcoverage-mapping (source-based coverage mapping is not yet implemented, \
                 a WP-941 follow-up; -fprofile-instr-generate counters work alone, and \
                 --coverage gives gcov line coverage)"
                    .into(),
            ));
        }
        if self.mapping == Some(true) && self.instr_generate.is_none() {
            e.push(ArgError::OnlyAllowedWith(
                "-fcoverage-mapping".into(),
                "-fprofile-instr-generate".into(),
            ));
        }
        if let Some(f) = &self.filter {
            if self.coverage {
                // `opt`'s `insert-gcov-profiling` takes no file filter.
                e.push(ArgError::UnsupportedByKiln(f.clone()));
            } else {
                e.push(ArgError::OnlyAllowedWith(f.clone(), "--coverage".into()));
            }
        }
        // `opt`'s gcov pass always writes both files: notes alone would
        // need a profile runtime the link does not add.
        if self.gcov_notes() && !self.gcov_data() {
            e.push(ArgError::UnsupportedByKiln(
                "-ftest-coverage without -fprofile-arcs (gcov notes without counters; \
                 use --coverage)"
                    .into(),
            ));
        }
        e
    }

    /// The `(notes, data)` files of a gcov compile of `input` (clang
    /// `addPGOAndCoverageFlags`): `${dumpdir}${basename(input)}` with
    /// `-dumpdir` (which a link defaults to `<output>-`, `a-` without
    /// `-o`), else the `-o` output, else the input's base name; made
    /// absolute against `cwd` and given the `.gcno` / `.gcda` extension.
    /// The data file is under `-fprofile-dir=` when one is given.
    pub fn gcov_files(
        &self,
        input: &str,
        output: Option<&str>,
        link: bool,
        cwd: &str,
    ) -> (String, String) {
        let base = Path::new(input)
            .file_name()
            .map_or_else(|| input.to_owned(), |f| f.to_string_lossy().into_owned());
        let dumpdir =
            self.dumpdir.clone().or_else(|| link.then(|| format!("{}-", output.unwrap_or("a"))));
        let mut name = match (dumpdir, output) {
            (Some(d), _) => format!("{d}{base}"),
            (None, Some(o)) => o.to_owned(),
            (None, None) => base,
        };
        if !name.starts_with('/') {
            name = format!("{}/{name}", cwd.trim_end_matches('/'));
        }
        let notes = replace_extension(&name, "gcno");
        let data = match &self.profile_dir {
            Some(d) if self.gcov_data() => {
                format!("{}/{}", d.trim_end_matches('/'), notes.trim_start_matches('/'))
            }
            _ => notes.clone(),
        };
        (notes, replace_extension(&data, "gcda"))
    }
}

/// `llvm::sys::path::replace_extension`: the file name's last extension
/// (after a `.` that does not start the name) replaced by `ext`.
fn replace_extension(path: &str, ext: &str) -> String {
    let file_at = path.rfind('/').map_or(0, |i| i + 1);
    let stem_end = match path[file_at..].rfind('.') {
        Some(i) if i > 0 || path[file_at..].len() > 1 => file_at + i,
        _ => path.len(),
    };
    // `.` and `..` have no extension.
    let file = &path[file_at..];
    let stem_end = if file == "." || file == ".." { path.len() } else { stem_end };
    format!("{}.{ext}", &path[..stem_end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> (CoverageFlags, Vec<Option<Result<(), ArgError>>>) {
        let mut f = CoverageFlags::default();
        let r = args.iter().map(|a| f.parse_arg(a)).collect();
        (f, r)
    }

    #[test]
    fn gcov_flags() {
        let (f, r) = parse(&["--coverage"]);
        assert_eq!(r, [Some(Ok(()))]);
        assert!(f.gcov_notes() && f.gcov_data() && f.errors().is_empty());
        assert_eq!(f.link_flags(), ["--coverage"]);
        let (f, _) = parse(&["-fprofile-arcs", "-ftest-coverage"]);
        assert!(f.gcov_notes() && f.gcov_data());
        let (f, _) = parse(&["-fprofile-arcs", "-fno-profile-arcs"]);
        assert!(!f.gcov());
        let (f, _) = parse(&["-ftest-coverage"]);
        assert!(matches!(&f.errors()[..], [ArgError::UnsupportedByKiln(_)]));
    }

    #[test]
    fn instr_flags() {
        let (f, _) = parse(&["-fprofile-instr-generate=x/%m.profraw", "-fcoverage-mapping"]);
        assert_eq!(f.instr_generate, Some(Some("x/%m.profraw".into())));
        assert!(f.mapping());
        assert!(matches!(&f.errors()[..], [ArgError::UnsupportedByKiln(m)]
            if m.contains("not yet implemented")));
        let (f, _) =
            parse(&["-fprofile-instr-generate", "-fcoverage-mapping", "-fno-coverage-mapping"]);
        assert!(!f.mapping() && f.errors().is_empty());
        assert_eq!(f.link_flags(), ["-fprofile-instr-generate"]);
        let (f, _) = parse(&["-fcoverage-mapping"]);
        assert_eq!(
            f.errors(),
            [ArgError::OnlyAllowedWith(
                "-fcoverage-mapping".into(),
                "-fprofile-instr-generate".into()
            )]
        );
        let (f, _) = parse(&["-fprofile-instr-generate", "-fno-profile-instr-generate"]);
        assert_eq!(f.instr_generate, None);
        let (_, r) = parse(&["-fprofile-update=bogus"]);
        assert_eq!(
            r,
            [Some(Err(ArgError::UnsupportedValue("-fprofile-update=".into(), "bogus".into())))]
        );
        // Observed but left to the debug and macro tables.
        let (f, r) = parse(&["-ffile-compilation-dir=/x", "-ffile-prefix-map=/a=/b"]);
        assert_eq!(r, [None, None]);
        assert_eq!(f.comp_dir.as_deref(), Some("/x"));
        assert_eq!(f.prefix_map, [("/a".to_owned(), "/b".to_owned())]);
    }

    /// Operator Q8: PGO is a known 1.0 limitation, named in the message.
    #[test]
    fn pgo_rejected() {
        for a in ["-fprofile-use", "-fprofile-use=x.prof", "-fprofile-instr-use=x"] {
            let (_, r) = parse(&[a]);
            let Some(Some(Err(ArgError::UnsupportedByKiln(m)))) = r.first().cloned() else {
                panic!("{a}")
            };
            assert!(m.starts_with(a) && m.contains("kiln 1.0 limitation"), "{m}");
        }
        let (f, r) = parse(&["-fprofile-generate=dir"]);
        assert_eq!(r, [Some(Ok(()))]);
        assert!(matches!(&f.errors()[..], [ArgError::UnsupportedByKiln(m)]
            if m.contains("IR profile-guided")));
    }

    #[test]
    fn gcov_file_names() {
        let cov = parse(&["--coverage"]).0;
        let cwd = "/w";
        assert_eq!(
            cov.gcov_files("d/a.cpp", Some("o/b.o"), false, cwd),
            ("/w/o/b.gcno".into(), "/w/o/b.gcda".into())
        );
        assert_eq!(
            cov.gcov_files("d/a.cpp", None, false, cwd),
            ("/w/a.gcno".into(), "/w/a.gcda".into())
        );
        assert_eq!(
            cov.gcov_files("a.cpp", Some("sub/prog.x"), true, cwd),
            ("/w/sub/prog.x-a.gcno".into(), "/w/sub/prog.x-a.gcda".into())
        );
        assert_eq!(cov.gcov_files("a.cpp", None, true, cwd).0, "/w/a-a.gcno");
        let pd = parse(&["--coverage", "-fprofile-dir=pd"]).0;
        assert_eq!(pd.gcov_files("a.cpp", Some("o/b.o"), false, cwd).1, "pd/w/o/b.gcda");
        assert_eq!(replace_extension("/w/-", "gcno"), "/w/-.gcno");
        assert_eq!(replace_extension("/w.d/x", "gcno"), "/w.d/x.gcno");
    }
}
