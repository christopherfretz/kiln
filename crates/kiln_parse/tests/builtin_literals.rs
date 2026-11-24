//! The builtin-macro literal table (spec/ast.md §22, spec/pp.md "Builtin
//! macros", WP-2821): a literal made by a builtin macro ([cpp.predefined],
//! clang's extensions) says which one, through `Ast::literal_builtin`,
//! whether the builtin is written directly, reached through a user macro,
//! passed as a macro argument, or pasted (`##`, [cpp.concat]) or
//! stringized (`#`, [cpp.stringize]) into the literal. Written literals
//! and units compiled without the option have none.

use std::sync::Arc;

use kiln_ast::Ast;
use kiln_ast::builtin_literals::BuiltinMacro;
use kiln_ast::dump::{DumpOptions, dump, written_exprs};
use kiln_ast::expr::ExprKind;
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_preprocessed};
use kiln_pp::{PpOptions, Preprocessor};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Each line holds one literal (or none); the expected builtin per line.
const SRC: &str = r#"#define LN __LINE__
#define FN __FILE__
#define ID(x) x
#define STR2(x) #x
#define STR(x) STR2(x)
#define CAT2(a, b) a##b
#define CAT(a, b) CAT2(a, b)
int line_direct = __LINE__;
int line_user = LN;
int line_arg = ID(__LINE__);
int line_user_in_arg = ID(ID(LN));
const char *file_direct = __FILE__;
const char *file_user = FN;
const char *file_arg = ID(__FILE__);
const char *file_name = __FILE_NAME__;
const char *base_file = __BASE_FILE__;
int counter = __COUNTER__;
int counter_arg = ID(__COUNTER__);
int include_level = __INCLUDE_LEVEL__;
const char *date = __DATE__;
const char *time_of_day = __TIME__;
const char *timestamp = __TIMESTAMP__;
int flt_eval_method = __FLT_EVAL_METHOD__;
const char *line_string = STR(__LINE__);
int line_pasted = CAT(1, __LINE__);
unsigned line_suffixed = CAT(__LINE__, u);
double line_floating = CAT(1., __LINE__);
const char *concatenated = "a" __FILE__ "b";
const char *concatenated_last = "a" "b" __FILE__;
int written = 42;
const char *written_string = "t.cpp";
const char *stringized_name = STR2(__LINE__);
int pasted_written = CAT(4, 2);
template <class T> int in_template() { return __LINE__; }
int instantiated = in_template<int>();
"#;

/// The builtin each line's literal came from (1-based lines; `None`: a
/// written literal, or no literal).
fn expected(line: u32) -> Option<BuiltinMacro> {
    use BuiltinMacro as B;
    Some(match line {
        8..=11 | 24..=27 | 34 => B::Line,
        12..=14 | 28 | 29 => B::File,
        15 => B::FileName,
        16 => B::BaseFile,
        17 | 18 => B::Counter,
        19 => B::IncludeLevel,
        20 => B::Date,
        21 => B::Time,
        22 => B::Timestamp,
        23 => B::FltEvalMethod,
        _ => return None,
    })
}

/// A literal of the written code: its line, its kind and its builtin.
type Lit = (u32, ExprKind, Option<BuiltinMacro>);

/// Parses `src` with the table on or off; calls `f` with the AST, the
/// literals of the written code and the AST dump.
fn with_literals<R>(src: &str, on: bool, f: impl FnOnce(&Ast, &[Lit], &str) -> R) -> R {
    let sm = SourceManager::new();
    let interner = Arc::new(Interner::new());
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions::default();
    let mut ppo = PpOptions::new(opts.std);
    ppo.parser_pragmas = true;
    ppo.builtin_literals = on;
    let mut pp = Preprocessor::new(&sm, &engine, &interner, ppo);
    pp.enter_main_file(file);
    let target = Target::for_arch(Arch::X86_64);
    let (tu, pp) = parse_preprocessed(
        pp,
        Arc::clone(&interner),
        &engine,
        target,
        &opts,
        ConstexprLimits::default(),
    );
    assert_eq!(pp.builtin_literals_on(), on);
    drop(pp);
    let errors: Vec<String> = sink
        .diagnostics()
        .iter()
        .filter(|d| d.level >= Level::Error)
        .map(|d| d.message.clone())
        .collect();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    let ast = &tu.ast;
    let mut lits = Vec::new();
    for e in written_exprs(ast) {
        let n = ast.exprs.get(e);
        let kind = n.kind();
        if !matches!(
            kind,
            ExprKind::IntegerLiteral
                | ExprKind::FloatingLiteral
                | ExprKind::CharacterLiteral
                | ExprKind::StringLiteral
        ) {
            continue;
        }
        let line = sm.line_col(sm.expansion_loc(n.loc())).expect("a file location").line;
        lits.push((line, kind, ast.literal_builtin(e)));
    }
    let mut buf = Vec::new();
    let dopts = DumpOptions { sources: Some(&sm), main_file: Some(file), ..DumpOptions::default() };
    dump(ast, &dopts, &mut buf).expect("dump to memory");
    f(ast, &lits, &String::from_utf8(buf).expect("utf-8 dump"))
}

#[test]
fn every_builtin_directly_through_a_user_macro_and_in_an_argument() {
    with_literals(SRC, true, |ast, lits, _| {
        let lines: Vec<u32> = lits.iter().map(|l| l.0).collect();
        for line in 8..=34 {
            assert!(lines.contains(&line), "no literal on line {line}: {lits:?}");
        }
        for &(line, kind, got) in lits {
            assert_eq!(got, expected(line), "line {line} ({kind:?})");
        }
        // The literals entered: one per marked written literal, plus the
        // instantiated copy of line 34's (the same location, one entry).
        let marked = lits.iter().filter(|l| l.2.is_some()).count();
        assert_eq!(ast.builtin_literals.len(), marked);
    });
}

#[test]
fn the_kinds_of_the_marked_literals() {
    with_literals(SRC, true, |_, lits, _| {
        let kind_of = |line: u32| lits.iter().find(|l| l.0 == line).map(|l| l.1);
        assert_eq!(kind_of(8), Some(ExprKind::IntegerLiteral));
        assert_eq!(kind_of(12), Some(ExprKind::StringLiteral));
        assert_eq!(kind_of(24), Some(ExprKind::StringLiteral), "stringized");
        assert_eq!(kind_of(26), Some(ExprKind::IntegerLiteral), "pasted with a suffix");
        assert_eq!(kind_of(27), Some(ExprKind::FloatingLiteral), "pasted into a floating literal");
    });
}

#[test]
fn off_by_default_nothing_is_recorded() {
    with_literals(SRC, false, |ast, lits, _| {
        assert!(ast.builtin_literals.is_empty());
        assert!(lits.iter().all(|l| l.2.is_none()), "{lits:?}");
    });
}

#[test]
fn the_dump_names_the_builtin() {
    with_literals(SRC, true, |_, _, d| {
        assert!(d.contains("IntegerLiteral 'int' 8 from __LINE__"), "{d}");
        assert!(d.contains("StringLiteral 'const char[6]' lvalue \"t.cpp\" from __FILE__"), "{d}");
        assert!(d.contains("IntegerLiteral 'int' 42\n"), "{d}");
    });
    with_literals(SRC, false, |_, _, d| assert!(!d.contains(" from __"), "{d}"));
}

#[test]
fn builtin_names_round_trip() {
    for b in BuiltinMacro::ALL {
        assert_eq!(BuiltinMacro::from_name(b.name()), Some(b));
    }
    assert_eq!(BuiltinMacro::from_name("__has_include"), None);
}
