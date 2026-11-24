//! WP-2824 (spec/modules.md §1, §2 item 0, §11.6; [module.unit],
//! [module.interface], [module.import]): a named module (or partition)
//! whose interface source is a buffer already in the source manager
//! (`ModuleOptions::module_buffers`) is imported exactly as the same text
//! named by `-fmodule-file=<name>=<path>` is, with nothing read from the
//! host: the same `ModuleTable`, owners and diagnostics but for the file
//! name. Every file of these translation units is in memory (an
//! `add_virtual` buffer, headers through an include resolver over
//! buffers), as tooling clients parse them.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use kiln_ast::Ast;
use kiln_diag::{DiagEngine, DiagId, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions};
use kiln_pp::{MemoryResolver, ModuleOptions, PpOptions, Preprocessor};
use kiln_sema::target::{Arch, Target};
use kiln_source::symbol::Interner;
use kiln_source::{SourceManager, WorkDir};

/// The interface of `m`.
const M: &str = "export module m;\nexport int f() { return 1; }\n";
/// `m`, re-exporting its partition `m:p`.
const M_WITH_P: &str = "export module m;\nexport import :p;\n";
/// The partition `m:p`.
const M_P: &str = "export module m:p;\nexport int g();\n";
/// A header imported as a header unit.
const H: &str = "#define X 1\n";

/// One parse: the main file `main.cpp`, the module sources
/// `(module key, file name, text)`, and how they are given.
struct Case<'a> {
    main: &'a str,
    modules: &'a [(&'a str, &'a str, &'a str)],
}

/// Where the module sources come from.
#[derive(Clone, Copy)]
enum Given<'a> {
    /// `add_virtual` buffers named by `module_buffers`.
    Buffers,
    /// Files written to this directory, named by `module_files`.
    Files(&'a Path),
}

/// What a parse produced, file names relative to the module sources'
/// directory: the units (key, kind, module, partition, imports), each
/// named declaration's owner, and the diagnostics.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    units: Vec<String>,
    owners: Vec<String>,
    diags: Vec<String>,
}

/// The run's options beyond the case.
#[derive(Clone, Copy, Default)]
struct Env<'a> {
    /// The source manager's working directory (decoys in test 6).
    work_dir: Option<&'a Path>,
    /// Also a `-fprebuilt-module-path` (`work_dir`) and a header-unit
    /// module file for `work_dir/h.h`: steps a buffer must come before.
    decoy_lookups: bool,
    /// Record a module file of the main unit.
    record: bool,
    /// `-fkiln-bmi-image` (no module file is read here: no change).
    image: bool,
}

fn parse(case: &Case<'_>, given: Given<'_>, env: Env<'_>) -> Outcome {
    let sm = SourceManager::new();
    if let Some(d) = env.work_dir {
        sm.set_work_dir(WorkDir::at(d).unwrap());
    }
    let main = sm.add_virtual("main.cpp", case.main.to_owned()).expect("location space");
    let mut module_buffers = Vec::new();
    let mut module_files = Vec::new();
    for &(key, name, text) in case.modules {
        match given {
            Given::Buffers => {
                let f = sm.add_virtual(name, text.to_owned()).expect("location space");
                module_buffers.push((key.to_owned(), f));
            }
            Given::Files(dir) => {
                let p = dir.join(name);
                std::fs::write(&p, text).unwrap();
                module_files.push((key.to_owned(), p));
            }
        }
    }
    let mut prebuilt_paths = Vec::new();
    if env.decoy_lookups
        && let Some(d) = env.work_dir
    {
        prebuilt_paths.push(d.to_path_buf());
        module_files.push((format!("\"{}", d.join("h.h").display()), d.join("h.pcm")));
    }
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions { std: kiln_lex::Std::Cxx20, ..ParseOptions::default() };
    let mut ppo = PpOptions::new(opts.std);
    ppo.parser_pragmas = true;
    ppo.modules = Some(ModuleOptions {
        module_files,
        module_buffers,
        prebuilt_paths,
        record: env.record,
        image: env.image,
        ..ModuleOptions::default()
    });
    let mut pp = Preprocessor::new(&sm, &engine, &interner, ppo);
    pp.set_include_resolver(Box::new(MemoryResolver::new().with_header("h.h", H)));
    pp.enter_main_file(main);
    let (tu, _pp) = kiln_parse::parse_preprocessed(
        pp,
        Arc::clone(&interner),
        &engine,
        Target::for_arch(Arch::X86_64),
        &opts,
        ConstexprLimits::default(),
    );
    let strip = |name: &str| match given {
        Given::Files(dir) => name
            .strip_prefix(&format!("{}/", dir.display()))
            .map_or_else(|| name.to_owned(), str::to_owned),
        Given::Buffers => name.to_owned(),
    };
    let diags = sink
        .take()
        .iter()
        .map(|d| {
            let at = sm.offset_of(d.span.lo).map_or_else(String::new, |(f, _)| {
                let lc = sm.line_col(d.span.lo).unwrap();
                format!("{}:{}:{}", strip(sm.file_name(f)), lc.line, lc.col)
            });
            format!("{at}: {:?} {:?}: {}", d.level, d.id, d.message)
        })
        .collect();
    Outcome { units: units(&tu.ast), owners: owners(&tu.ast), diags }
}

/// Each unit as `key kind module partition imports`.
fn units(ast: &Ast) -> Vec<String> {
    ast.modules
        .units
        .iter()
        .map(|u| {
            format!("{:?} {:?} {:?} {:?} {:?}", u.key, u.kind, u.module, u.partition, u.imports)
        })
        .collect()
}

/// Each named declaration of a unit other than 0, or in a purview, as
/// `name unit purview exported`.
fn owners(ast: &Ast) -> Vec<String> {
    ast.decls
        .ids()
        .filter_map(|d| {
            let o = ast.modules.owner(d);
            let name = ast.decls.get(d).name().as_ident()?;
            (o.unit != 0 || o.purview).then(|| {
                let name = ast.symbols().as_str(name);
                format!("{name} {} {} {}", o.unit, o.purview, o.exported)
            })
        })
        .collect()
}

/// A fresh directory under the test target's temp directory.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("kiln-module-buffers-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(&dir).unwrap()
}

/// Test 1 (a buffer module).
const IMPORT_M: Case<'static> =
    Case { main: "import m;\nint y = f();\n", modules: &[("m", "m.cppm", M)] };
/// Test 2 (a partition).
const IMPORT_P: Case<'static> = Case {
    main: "import m;\nint y = g();\n",
    modules: &[("m", "m.cppm", M_WITH_P), ("m:p", "m-p.cppm", M_P)],
};
/// Test 3 (an implementation unit).
const IMPL_M: Case<'static> =
    Case { main: "module m;\nint h() { return f(); }\n", modules: &[("m", "m.cppm", M)] };
/// Test 5 (an error in a buffer module).
const BROKEN: Case<'static> = Case {
    main: "import m;\nint y = f();\n",
    modules: &[("m", "m.cppm", "export module m;\nexport int f() { return nope; }\n")],
};
/// Test 6's header-unit import.
const HEADER: Case<'static> =
    Case { main: "import \"h.h\";\nimport m;\nint y = f() + X;\n", modules: &[("m", "m.cppm", M)] };

/// The owner line of `name`.
fn owner_of<'a>(o: &'a Outcome, name: &str) -> &'a str {
    let pre = format!("{name} ");
    o.owners.iter().find(|l| l.starts_with(&pre)).unwrap_or_else(|| panic!("{name}: {o:?}"))
}

/// Test 1, [module.interface]/1, [module.import]/2: `import m;` of a
/// buffer builds unit 1 `Interface` `m`; `f` is owned by it, in its
/// purview, exported.
#[test]
fn module_buffer_import_module_import() {
    let o = parse(&IMPORT_M, Given::Buffers, Env::default());
    assert!(o.diags.is_empty(), "{o:?}");
    assert_eq!(o.units[0], "\"\" NonModule None None [(\"m\", false)]");
    assert_eq!(o.units[1], "\"m\" Interface Some(\"m\") None []");
    assert_eq!(owner_of(&o, "f"), "f 1 true true");
}

/// Test 2, [module.unit]/3, [module.import]/7: a partition given as a
/// buffer is found by the interface's `export import :p;` and `g` is
/// visible to the importer of `m`.
#[test]
fn module_buffer_partition_module_unit() {
    let o = parse(&IMPORT_P, Given::Buffers, Env::default());
    assert!(o.diags.is_empty(), "{o:?}");
    assert_eq!(o.units.len(), 3, "{o:?}");
    assert_eq!(o.units[1], "\"m\" Interface Some(\"m\") None [(\"m:p\", true)]");
    assert_eq!(o.units[2], "\"m:p\" PartitionInterface Some(\"m\") Some(\"p\") []");
    assert_eq!(owner_of(&o, "g"), "g 2 true true");
}

/// Test 3, [module.unit]/8: `module m;` as the main file implicitly
/// imports `m`, here a buffer.
#[test]
fn module_buffer_implementation_unit_module_unit() {
    let o = parse(&IMPL_M, Given::Buffers, Env::default());
    assert!(o.diags.is_empty(), "{o:?}");
    assert_eq!(o.units[0], "\"\" Implementation Some(\"m\") None [(\"m\", false)]");
    assert_eq!(o.units[1], "\"m\" Interface Some(\"m\") None []");
    assert_eq!(owner_of(&o, "f"), "f 1 true true");
    assert_eq!(owner_of(&o, "h"), "h 0 true false");
}

/// Test 4 (spec/modules.md §2 items 0 and 1): the same text named by
/// `-fmodule-file=<name>=<path>` gives the same `ModuleTable`, owners
/// and diagnostics, file names aside.
#[test]
fn module_buffer_same_as_module_file_module_import() {
    let dir = temp_dir("files");
    for case in [&IMPORT_M, &IMPORT_P, &IMPL_M, &BROKEN] {
        let from_buffers = parse(case, Given::Buffers, Env::default());
        let from_files = parse(case, Given::Files(&dir), Env::default());
        assert_eq!(from_buffers, from_files);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 5: an error in a module given as a buffer is reported in the
/// buffer, by its name.
#[test]
fn module_buffer_error_names_the_buffer_module_import() {
    let o = parse(&BROKEN, Given::Buffers, Env::default());
    assert_eq!(o.diags.len(), 1, "{o:?}");
    assert!(o.diags[0].starts_with("m.cppm:2:"), "{o:?}");
    assert!(o.diags[0].contains("'nope'"), "{o:?}");
}

/// Test 6 (spec/modules.md §2 item 0, §11.6): a working directory
/// holding a different `h.h`, `m.cppm` and `m-p.cppm`, also a prebuilt
/// module path, and a header-unit module file named for its `h.h`,
/// change nothing: no host lookup stands for a buffer. Also with
/// `-fkiln-bmi-image`.
#[test]
fn module_buffer_ignores_the_host_module_import() {
    let empty = temp_dir("empty");
    let decoys = temp_dir("decoys");
    // Each decoy declares other names: taking one shows as a diagnostic.
    std::fs::write(decoys.join("h.h"), "#define DECOY_X 2\n").unwrap();
    std::fs::write(decoys.join("m.cppm"), "export module m;\nexport int decoy_f();\n").unwrap();
    std::fs::write(decoys.join("m-p.cppm"), "export module m:p;\nexport int decoy_g();\n").unwrap();
    for case in [&IMPORT_M, &IMPORT_P, &IMPL_M, &HEADER] {
        let want = parse(case, Given::Buffers, Env { work_dir: Some(&empty), ..Env::default() });
        assert!(want.diags.is_empty(), "{want:?}");
        for image in [false, true] {
            let env = Env { work_dir: Some(&decoys), decoy_lookups: true, image, ..Env::default() };
            assert_eq!(parse(case, Given::Buffers, env), want);
        }
    }
    let _ = std::fs::remove_dir_all(&empty);
    let _ = std::fs::remove_dir_all(&decoys);
}

/// spec/modules.md §2 item 0: a buffer is always a source, whatever its
/// name or first bytes; neither a `.pcm` name nor a module file's magic
/// makes it read as a module file.
#[test]
fn module_buffer_is_always_a_source_module_import() {
    let pcm = Case { main: IMPORT_M.main, modules: &[("m", "m.pcm", M)] };
    let o = parse(&pcm, Given::Buffers, Env::default());
    assert!(o.diags.is_empty(), "{o:?}");
    assert_eq!(owner_of(&o, "f"), "f 1 true true");
    for magic in ["KILNBMI\0", "CPCH"] {
        let text = format!("{magic}\n{M}");
        let case = Case { main: IMPORT_M.main, modules: &[("m", "m.pcm", &text)] };
        let o = parse(&case, Given::Buffers, Env::default());
        assert!(o.units.iter().any(|u| u.starts_with("\"m\" ")), "{o:?}");
        let module_file = [
            DiagId::pp_module_file_clang,
            DiagId::pp_not_module_file,
            DiagId::pp_module_file_corrupt,
            DiagId::pp_module_file_format,
            DiagId::pp_file_not_found,
        ];
        for id in module_file {
            let id = format!("Some({id:?})");
            assert!(!o.diags.iter().any(|d| d.contains(&id)), "{o:?}");
        }
    }
}

/// spec/modules.md §2 item 0: a module file recorded for the main unit
/// cannot name a buffer as its import's path: refused, the unit still
/// built; without recording, no diagnostic.
#[test]
fn module_buffer_refused_under_record_module_import() {
    let case = Case {
        main: "export module n;\nimport m;\nexport int z() { return f(); }\n",
        modules: &[("m", "m.cppm", M)],
    };
    let o = parse(&case, Given::Buffers, Env::default());
    assert!(o.diags.is_empty(), "{o:?}");
    let o = parse(&case, Given::Buffers, Env { record: true, ..Env::default() });
    assert_eq!(o.diags.len(), 1, "{o:?}");
    assert!(o.diags[0].starts_with("main.cpp:2:8: Error Some(pp_module_buffer_recorded)"), "{o:?}");
    assert!(o.diags[0].contains("module 'm' is built from the in-memory buffer 'm.cppm'"), "{o:?}");
    assert_eq!(owner_of(&o, "f"), "f 1 true true");
}
