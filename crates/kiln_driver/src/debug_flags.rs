//! The debug information flags (spec/driver.md "Debug information
//! flags"), as clang 19's driver reads them on Linux: the last `-g`
//! level wins, `-gdwarf-N` also enables `-g`, and each flag kiln does not
//! implement is rejected by name rather than ignored.

use kiln_codegen::{DebugInfoKind, DebugOptions};

use crate::args::ArgError;

/// `-gz` / `-gz=<type>`: compressed debug sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugCompression {
    /// `-gz=none`.
    None,
    /// `-gz`, `-gz=zlib`.
    Zlib,
    /// `-gz=zstd`.
    Zstd,
}

impl DebugCompression {
    /// The `--compress-debug-sections=` value.
    pub fn name(self) -> &'static str {
        match self {
            DebugCompression::None => "none",
            DebugCompression::Zlib => "zlib",
            DebugCompression::Zstd => "zstd",
        }
    }
}

/// The debug flags of a command line.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugFlags {
    /// The last level flag (`-g`, `-g0..3`, `-ggdb*`, `-gline-tables-only`,
    /// `-gmlt`, `-gline-directives-only`, `-gdwarf*`): `None` if none.
    pub level: Option<DebugInfoKind>,
    /// The last `-gdwarf-N` version (`-gdwarf` alone: the default).
    pub dwarf_version: Option<u32>,
    /// `-gno-column-info` was last.
    pub no_column_info: bool,
    /// The last of `-fstandalone-debug` / `-fno-standalone-debug`
    /// (`-fno-limit-debug-info` / `-flimit-debug-info`).
    pub standalone: Option<bool>,
    /// `-fno-eliminate-unused-debug-types` was last.
    pub unused_types: bool,
    /// `-fdebug-prefix-map=` and `-ffile-prefix-map=` entries, in order
    /// (`crate::args` adds the second, which also maps macros).
    pub prefix_map: Vec<(String, String)>,
    /// The last `-fdebug-compilation-dir=` / `-ffile-compilation-dir=`.
    pub comp_dir: Option<String>,
    /// The last of `-gsplit-dwarf[=split]` / `-gno-split-dwarf`.
    pub split_dwarf: bool,
    /// The last `-gz[=<type>]`.
    pub compress: Option<DebugCompression>,
}

/// Flags accepted with no effect: each spells clang's default on Linux.
const DEFAULTS: &[&str] = &[
    "-gcolumn-info",
    "-gdwarf32",
    "-gno-embed-source",
    "-gno-codeview",
    "-gno-pubnames",
    "-gno-gnu-pubnames",
    "-gno-simple-template-names",
    "-gno-record-command-line",
    "-gno-record-gcc-switches",
    "-gno-strict-dwarf",
    "-gno-modules",
    "-gno-inline-line-tables",
    "-gno-template-alias",
    "-feliminate-unused-debug-types",
    "-fno-debug-macro",
    "-fno-debug-types-section",
    "-fno-split-dwarf-inlining",
    "-fdwarf-directory-asm",
    "-fno-dwarf-directory-asm",
    "-ggnu-pubnames-none",
    "-gsplit-dwarf=split",
];

/// Flags rejected: "'<flag>' is not supported by kiln".
const REJECTED: &[&str] = &[
    "-gembed-source",
    "-gcodeview",
    "-gcodeview-ghash",
    "-gpubnames",
    "-ggnu-pubnames",
    "-gsimple-template-names",
    "-gsimple-template-names=simple",
    "-gsimple-template-names=mangled",
    "-grecord-command-line",
    "-grecord-gcc-switches",
    "-gstrict-dwarf",
    "-gdwarf64",
    "-gmodules",
    "-glldb",
    "-gsce",
    "-gdbx",
    "-gtemplate-alias",
    "-fdebug-macro",
    "-fdebug-types-section",
    "-fsplit-dwarf-inlining",
    "-fdebug-info-for-profiling",
    "-gsplit-dwarf=single",
    "-ginline-line-tables",
];

impl DebugFlags {
    /// Takes `arg` if it is a debug flag: `None` if it is not, else
    /// whether it is accepted.
    pub fn parse_arg(&mut self, arg: &str) -> Option<Result<(), ArgError>> {
        let level = match arg {
            "-g" | "-g2" | "-g3" | "-ggdb" | "-ggdb2" | "-ggdb3" | "-gfull" | "-gused" => {
                Some(DebugInfoKind::Constructor)
            }
            "-g0" | "-ggdb0" => Some(DebugInfoKind::None),
            "-g1" | "-ggdb1" | "-gmlt" | "-gline-tables-only" => Some(DebugInfoKind::LineTablesOnly),
            "-gline-directives-only" => Some(DebugInfoKind::LineDirectivesOnly),
            _ => None,
        };
        if let Some(l) = level {
            self.level = Some(l);
            return Some(Ok(()));
        }
        match arg {
            "-gdwarf" => self.set_dwarf(None),
            "-gdwarf-2" => self.set_dwarf(Some(2)),
            "-gdwarf-3" => self.set_dwarf(Some(3)),
            "-gdwarf-4" => self.set_dwarf(Some(4)),
            "-gdwarf-5" => self.set_dwarf(Some(5)),
            "-gno-column-info" => self.no_column_info = true,
            "-gcolumn-info" => self.no_column_info = false,
            "-fstandalone-debug" | "-fno-limit-debug-info" => self.standalone = Some(true),
            "-fno-standalone-debug" | "-flimit-debug-info" => self.standalone = Some(false),
            "-fno-eliminate-unused-debug-types" => self.unused_types = true,
            "-feliminate-unused-debug-types" => self.unused_types = false,
            "-gsplit-dwarf" | "-gsplit-dwarf=split" => self.split_dwarf = true,
            "-gno-split-dwarf" => self.split_dwarf = false,
            "-gz" => self.compress = Some(DebugCompression::Zlib),
            _ if DEFAULTS.contains(&arg) => {}
            _ if REJECTED.contains(&arg) => {
                return Some(Err(ArgError::UnsupportedByKiln(arg.to_owned())));
            }
            _ => return self.valued(arg),
        }
        Some(Ok(()))
    }

    fn set_dwarf(&mut self, v: Option<u32>) {
        // clang: `-gdwarf-N` is in `g_Group` and turns debug information on.
        self.level = Some(DebugInfoKind::Constructor);
        self.dwarf_version = Some(v.unwrap_or(5));
    }

    /// The `=`-valued flags.
    fn valued(&mut self, arg: &str) -> Option<Result<(), ArgError>> {
        let (flag, v) = arg.split_once('=')?;
        match flag {
            "-fdebug-prefix-map" => match v.split_once('=') {
                Some((old, new)) => self.prefix_map.push((old.to_owned(), new.to_owned())),
                None => {
                    return Some(Err(ArgError::InvalidValue(format!("{flag}="), v.to_owned())));
                }
            },
            "-fdebug-compilation-dir" | "-ffile-compilation-dir" => {
                self.comp_dir = Some(v.to_owned());
            }
            "-gz" => {
                self.compress = Some(match v {
                    "none" => DebugCompression::None,
                    "zlib" => DebugCompression::Zlib,
                    "zstd" => DebugCompression::Zstd,
                    _ => {
                        return Some(Err(ArgError::UnsupportedValue("-gz=".into(), v.into())));
                    }
                });
            }
            "-fdebug-default-version" => match v.parse::<u32>() {
                Ok(n @ 2..=5) => {
                    if self.dwarf_version.is_none() {
                        self.dwarf_version = Some(n);
                    }
                }
                _ => return Some(Err(ArgError::InvalidFlagValue(arg.to_owned(), v.to_owned()))),
            },
            _ => return None,
        }
        Some(Ok(()))
    }

    /// The debug information kind in effect: `-fstandalone-debug` turns
    /// clang's Linux default `constructor` into `full`;
    /// `-fno-eliminate-unused-debug-types` into `unused-types`.
    pub fn kind(&self) -> DebugInfoKind {
        match self.level.unwrap_or(DebugInfoKind::None) {
            DebugInfoKind::Constructor | DebugInfoKind::Limited | DebugInfoKind::Full
                if self.unused_types =>
            {
                DebugInfoKind::UnusedTypes
            }
            DebugInfoKind::Constructor if self.standalone == Some(true) => DebugInfoKind::Full,
            k => k,
        }
    }

    /// The codegen options: `cwd` is the compilation directory unless
    /// `-fdebug-compilation-dir=` names one; `split_file` is the `.dwo`
    /// name under `-gsplit-dwarf`.
    pub fn options(&self, cwd: &str, split_file: Option<String>) -> DebugOptions {
        let kind = self.kind();
        let on = kind != DebugInfoKind::None;
        DebugOptions {
            kind,
            // clang passes `-dwarf-version=` whenever debug information is
            // on or a version was given; the module flag follows it.
            dwarf_version: if on { Some(self.dwarf_version.unwrap_or(5)) } else { self.dwarf_version },
            column_info: !self.no_column_info,
            prefix_map: self.prefix_map.clone(),
            comp_dir: self.comp_dir.clone().unwrap_or_else(|| cwd.to_owned()),
            split_dwarf_file: split_file.filter(|_| on && self.split_dwarf),
            ..DebugOptions::default()
        }
    }

    /// The `-gz` compression in effect (`-gz=none` is none).
    pub fn compression(&self) -> Option<DebugCompression> {
        self.compress.filter(|z| *z != DebugCompression::None)
    }

    /// Whether `-gsplit-dwarf` is in effect (debug information on).
    pub fn split(&self) -> bool {
        self.split_dwarf && self.kind() != DebugInfoKind::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> (DebugFlags, Vec<Option<Result<(), ArgError>>>) {
        let mut f = DebugFlags::default();
        let r = args.iter().map(|a| f.parse_arg(a)).collect();
        (f, r)
    }

    #[test]
    fn levels_last_wins() {
        assert_eq!(parse(&["-g"]).0.kind(), DebugInfoKind::Constructor);
        assert_eq!(parse(&["-g", "-g0"]).0.kind(), DebugInfoKind::None);
        assert_eq!(parse(&["-g0", "-ggdb3"]).0.kind(), DebugInfoKind::Constructor);
        assert_eq!(parse(&["-g", "-gmlt"]).0.kind(), DebugInfoKind::LineTablesOnly);
        assert_eq!(parse(&["-g1"]).0.kind(), DebugInfoKind::LineTablesOnly);
        assert_eq!(
            parse(&["-gline-directives-only"]).0.kind(),
            DebugInfoKind::LineDirectivesOnly
        );
        assert_eq!(parse(&[]).0.kind(), DebugInfoKind::None);
        assert_eq!(parse(&["-g", "-fstandalone-debug"]).0.kind(), DebugInfoKind::Full);
        assert_eq!(
            parse(&["-g", "-fstandalone-debug", "-flimit-debug-info"]).0.kind(),
            DebugInfoKind::Constructor
        );
        assert_eq!(
            parse(&["-g", "-fno-eliminate-unused-debug-types"]).0.kind(),
            DebugInfoKind::UnusedTypes
        );
    }

    #[test]
    fn dwarf_versions() {
        let (f, r) = parse(&["-gdwarf-4"]);
        assert_eq!(r, [Some(Ok(()))]);
        assert_eq!(f.kind(), DebugInfoKind::Constructor);
        assert_eq!(f.options("/w", None).dwarf_version, Some(4));
        assert_eq!(parse(&["-g"]).0.options("/w", None).dwarf_version, Some(5));
        assert_eq!(parse(&["-gdwarf"]).0.options("/w", None).dwarf_version, Some(5));
        // clang: `-gdwarf-4 -g0` keeps `-dwarf-version=4` with no debug info.
        let f = parse(&["-gdwarf-4", "-g0"]).0;
        assert_eq!(f.kind(), DebugInfoKind::None);
        assert_eq!(f.options("/w", None).dwarf_version, Some(4));
        assert_eq!(parse(&[]).0.options("/w", None).dwarf_version, None);
    }

    #[test]
    fn paths_and_columns() {
        let (f, r) = parse(&[
            "-g",
            "-gno-column-info",
            "-fdebug-prefix-map=/a=/b",
            "-fdebug-prefix-map=/c=",
            "-fdebug-compilation-dir=.",
        ]);
        assert!(r.iter().all(|x| *x == Some(Ok(()))));
        let o = f.options("/w", None);
        assert!(!o.column_info);
        assert_eq!(o.prefix_map, [("/a".into(), "/b".into()), ("/c".into(), String::new())]);
        assert_eq!(o.comp_dir, ".");
        assert_eq!(parse(&["-g"]).0.options("/w", None).comp_dir, "/w");
        assert_eq!(
            parse(&["-fdebug-prefix-map=/a"]).1,
            [Some(Err(ArgError::InvalidValue("-fdebug-prefix-map=".into(), "/a".into())))]
        );
    }

    #[test]
    fn split_and_compression() {
        let f = parse(&["-g", "-gsplit-dwarf"]).0;
        assert!(f.split());
        assert_eq!(f.options("/w", Some("t.dwo".into())).split_dwarf_file.as_deref(), Some("t.dwo"));
        assert!(!parse(&["-gsplit-dwarf"]).0.split());
        assert!(!parse(&["-g", "-gsplit-dwarf", "-gno-split-dwarf"]).0.split());
        assert_eq!(parse(&["-gz"]).0.compress, Some(DebugCompression::Zlib));
        assert_eq!(parse(&["-gz=zstd"]).0.compress, Some(DebugCompression::Zstd));
        assert_eq!(parse(&["-gz=none"]).0.compress, Some(DebugCompression::None));
        assert_eq!(
            parse(&["-gz=lzma"]).1,
            [Some(Err(ArgError::UnsupportedValue("-gz=".into(), "lzma".into())))]
        );
    }

    #[test]
    fn rejected_and_unknown() {
        for a in ["-gembed-source", "-gcodeview", "-gpubnames", "-ggnu-pubnames", "-fdebug-macro"] {
            assert_eq!(parse(&[a]).1, [Some(Err(ArgError::UnsupportedByKiln(a.into())))], "{a}");
        }
        assert_eq!(
            parse(&["-gsimple-template-names"]).1,
            [Some(Err(ArgError::UnsupportedByKiln("-gsimple-template-names".into())))]
        );
        assert_eq!(parse(&["-gno-pubnames", "-gcolumn-info"]).1, [Some(Ok(())), Some(Ok(()))]);
        assert_eq!(parse(&["-gfoo"]).1, [None]);
        assert_eq!(parse(&["-O2"]).1, [None]);
    }
}
