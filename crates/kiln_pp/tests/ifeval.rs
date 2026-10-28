//! WP-015 per-rule tests for [`kiln_pp::IfEval`] ([cpp.cond]) and
//! [`kiln_pp::ClangFeatures`]: hand-checked expectations (each agrees with
//! clang 19), runnable without clang. The exhaustive comparison with clang
//! is `ifeval_differential.rs` and `features.rs`.

mod ifeval_common;

use ifeval_common::{run_kiln, words};
use kiln_lex::Std;
use kiln_pp::{ClangFeatures, MemoryResolver};

const X86: &str = "x86_64-unknown-linux-gnu";
const ARM: &str = "aarch64-unknown-linux-gnu";

/// Preprocesses `src` for `triple` (C++26, clang's predefines, the
/// feature tables) with diagnostic arguments `args`; returns the output
/// words and the diagnostics.
fn pp_on(triple: &str, src: &str, args: &[&str]) -> (Vec<String>, Vec<String>) {
    pp_with(triple, src, args, MemoryResolver::new())
}

fn pp_with(
    triple: &str,
    src: &str,
    args: &[&str],
    r: MemoryResolver,
) -> (Vec<String>, Vec<String>) {
    let out = run_kiln(src, triple, Std::Cxx26, args, r, |pp| {
        pp.set_feature_queries(Box::new(ClangFeatures::new(triple, Std::Cxx26, &[]).unwrap()));
    });
    (words(&out.text).into_iter().map(str::to_owned).collect(), out.diags)
}

fn pp(src: &str) -> (Vec<String>, Vec<String>) {
    pp_on(X86, src, &[])
}

/// The words of `src` and the assertion that it is diagnostic-free.
fn clean(src: &str) -> Vec<String> {
    let (w, d) = pp(src);
    assert_eq!(d, [] as [String; 0], "{src}: {d:?}");
    w
}

/// `#if <e>` selects `yes` (or `no`); diagnostics are not checked.
fn truth(e: &str) -> bool {
    let w = pp(&format!("#if {e}\nyes\n#else\nno\n#endif\n")).0;
    match w.as_slice() {
        [y] if y == "yes" => true,
        [n] if n == "no" => false,
        _ => panic!("{e}: {w:?}"),
    }
}

/// [cpp.cond]/13: the controlling expression is evaluated in `intmax_t`
/// and `uintmax_t` (64 bits); a signed operand is converted to unsigned
/// by the usual arithmetic conversions.
#[test]
fn intmax_arithmetic_and_conversions() {
    assert!(truth("0x7fffffffffffffff == 9223372036854775807"));
    assert!(truth("18446744073709551615u == -1"));
    assert!(truth("1u - 2 > 0"));
    assert!(truth("-1 / 2u > 1000"));
    assert!(truth("(1 ? -1 : 0u) > 0"));
    assert!(truth("-10 / 3 == -3 && -10 % 3 == -1"));
    assert!(truth("1 << 63 < 0"));
    assert!(truth("-1 >> 1 == -1"), "arithmetic right shift of signed");
    assert!(truth("0xffffffffffffffff >> 63 == 1"));
    assert!(!truth("-1 < 0u"));
    let (_, d) = pp("#if -1 < 0u\n#endif\n");
    assert_eq!(
        d,
        [
            "1:8: warning: left side of operator converted from negative value to unsigned: -1 to 18446744073709551615"
        ]
    );
}

/// [cpp.cond]/13: signed overflow is diagnosed (clang: a warning) and wraps.
#[test]
fn signed_overflow() {
    let (w, d) = pp("#if 0x7fffffffffffffff + 1 < 0\nwrapped\n#endif\n");
    assert_eq!(w, ["wrapped"]);
    assert_eq!(d, ["1:24: warning: integer overflow in preprocessor expression"]);
    assert!(
        clean("#if 0xffffffffffffffff + 1 == 0\nok\n#endif\n") == ["ok"],
        "unsigned wraps silently"
    );
}

/// [cpp.cond]/13, [expr.mul]/4: division by zero is an error, and the
/// directive's group is skipped.
#[test]
fn division_by_zero() {
    let (w, d) = pp("#if 1 / 0\nbad\n#else\nelse\n#endif\n");
    assert_eq!(d, ["1:7: error: division by zero in preprocessor expression"]);
    assert!(!w.contains(&"bad".to_owned()));
    let (_, d) = pp("#if 1 % 0\n#endif\n");
    assert_eq!(d, ["1:7: error: remainder by zero in preprocessor expression"]);
}

/// [cpp.cond]/13, [expr.log.and], [expr.log.or], [expr.cond]: no
/// diagnostics in an unevaluated operand.
#[test]
fn unevaluated_operands_are_silent() {
    for e in [
        "1 || 1 / 0",
        "!(0 && 1 / 0)",
        "1 ? 1 : 1 / 0",
        "0 ? 1 / 0 : 1",
        "!(0 && (0x7fffffffffffffff + 1))",
        "1 || (-1 < 0u)",
        "(0 && (1, 2)) == 0",
    ] {
        assert_eq!(clean(&format!("#if {e}\nyes\n#endif\n")), ["yes"], "{e}");
    }
}

/// [cpp.cond]/13: identifiers other than `true` and `false` are 0 after
/// macro replacement; `-Wundef` reports them.
#[test]
fn identifiers_are_zero() {
    assert!(truth("!undefined_name"));
    assert!(truth("true && !false"));
    let (_, d) = pp_on(X86, "#if FOO\n#endif\n", &["-Wundef"]);
    assert_eq!(d, ["1:5: warning: 'FOO' is not defined, evaluates to 0 [-Wundef]"]);
}

/// [cpp.cond]/13: character literals have their execution-character-set
/// values; the signedness of `char` is the target's.
#[test]
fn character_literals() {
    assert!(truth("'A' == 65 && '\\n' == 10 && '\\0' == 0"));
    assert!(truth("u8'a' == 97 && u'\\u00e9' == 0xe9 && U'\\U0001F600' == 0x1F600"));
    assert!(truth("L'\\xff' == 255"));
    let w = |t| {
        let (w, d) = pp_on(t, "#if '\\377' < 0\nsigned\n#else\nunsigned\n#endif\n", &[]);
        assert_eq!(d, [] as [String; 0]);
        w
    };
    assert_eq!(w(X86), ["signed"]);
    assert_eq!(w(ARM), ["unsigned"]);
    let (_, d) = pp("#if 'ab' == 24930\n#endif\n");
    assert_eq!(d, ["1:5: warning: multi-character character constant [-Wmultichar]"]);
}

/// [cpp.cond]/1, /13: the controlling expression must be a complete
/// constant expression; clang's wording for malformed ones.
#[test]
fn malformed_expressions() {
    for (src, diag) in [
        ("#if 1 +\n#endif\n", "1:8: error: expected value in expression"),
        ("#if 1.0\n#endif\n", "1:5: error: floating point literal in preprocessor expression"),
        ("#if \"s\"\n#endif\n", "1:5: error: invalid token at start of a preprocessor expression"),
        (
            "#if 1 2\n#endif\n",
            "1:7: error: token is not a valid binary operator in a preprocessor subexpression",
        ),
        (
            "#if 1 = 1\n#endif\n",
            "1:7: error: token is not a valid binary operator in a preprocessor subexpression",
        ),
    ] {
        let (_, d) = pp(src);
        assert_eq!(d, [diag], "{src}");
    }
    let (_, d) = pp("#if (1\n#endif\n");
    assert_eq!(
        d,
        ["1:7: error: expected ')' in preprocessor expression", "1:5: note: to match this '('"]
    );
    let (_, d) = pp("#if 1 ? 2\n#endif\n");
    assert_eq!(d, ["1:10: error: expected ':'", "1:7: note: to match this '?'"]);
}

/// [cpp.cond]/13 (with [expr.comma]): a top-level comma ends the
/// expression in C++ (clang's error); inside an unevaluated operand it is
/// accepted (see `unevaluated_operands_are_silent`).
#[test]
fn comma_operator() {
    let (_, d) = pp("#if 1, 2\n#endif\n");
    assert_eq!(d, ["1:6: error: expected end of line in preprocessor expression"]);
}

/// [cpp.cond]/10: `defined X` and `defined(X)`; malformed operands.
#[test]
fn defined_operator() {
    assert_eq!(clean("#define X\n#if defined X && defined(X) && !defined Y\nok\n#endif\n"), ["ok"]);
    assert!(truth("defined __cplusplus && defined(__has_include)"));
    let (_, d) = pp("#if defined\n#endif\n");
    assert_eq!(d, ["1:12: error: macro name missing"]);
    let (_, d) = pp("#if defined(X\n#endif\n");
    assert_eq!(d, ["1:14: error: missing ')' after 'defined'", "1:12: note: to match this '('"]);
    let (_, d) = pp("#if defined 1\n#endif\n");
    assert_eq!(d, ["1:13: error: macro name must be an identifier"]);
}

/// [cpp.cond]/10: `defined` produced by macro replacement is undefined
/// behaviour; clang evaluates it and warns (`-Wexpansion-to-defined`
/// under `-pedantic` for the object-like form).
#[test]
fn defined_from_macro() {
    let (w, d) = pp_on(
        X86,
        "#define D defined(X)\n#define X\n#if D\nok\n#endif\n",
        &["-Wexpansion-to-defined"],
    );
    assert_eq!(w, ["ok"]);
    assert_eq!(
        d,
        [
            "3:5: warning: macro expansion producing 'defined' has undefined behavior [-Wexpansion-to-defined]"
        ]
    );
}

/// [cpp.cond]/2-4: `__has_include` searches like `#include`, also with a
/// macro-expanded operand, and is usable in `#elif`.
#[test]
fn has_include() {
    let r = MemoryResolver::new().with_header("a.h", "").with_system_header("s.h", "");
    let src = "#define S <s.h>\n#if __has_include(\"a.h\") && __has_include(<s.h>) && __has_include(S)\nall\n#endif\n\
               #if 0\n#elif !__has_include(\"missing.h\")\nnot_found\n#endif\n";
    let (w, d) = pp_with(X86, src, &[], r);
    assert_eq!(d, [] as [String; 0], "{d:?}");
    assert_eq!(w, ["all", "not_found"]);
}

/// [cpp.cond]/5, Table 23: `__has_cpp_attribute` gives the standard
/// attributes' dates and is 0 for unknown ones; the value prints with an
/// `L` suffix.
#[test]
fn has_cpp_attribute() {
    let w = clean(
        "__has_cpp_attribute(nodiscard) __has_cpp_attribute(fallthrough) \
         __has_cpp_attribute(no_such) __has_cpp_attribute(gnu::always_inline)\n",
    );
    assert_eq!(w, ["201907L", "201603L", "0", "1"]);
    assert!(truth("__has_cpp_attribute(likely) >= 201803L"));
}

/// [cpp.cond]/5-7: the clang `__has_*` extensions are answered by the
/// feature tables for the configured target.
#[test]
fn clang_feature_queries() {
    assert!(truth("__has_feature(cxx_rtti) && __has_extension(cxx_exceptions)"));
    assert!(truth("__has_builtin(__builtin_expect) && !__has_builtin(__builtin_nonexistent)"));
    assert!(truth("__has_attribute(always_inline) && __has_attribute(__always_inline__)"));
    assert!(truth("__is_identifier(foo) && !__is_identifier(int)"));
    assert!(truth(
        "__is_target_arch(x86_64) && __is_target_os(linux) && __is_target_vendor(unknown)"
    ));
    assert!(truth("__has_constexpr_builtin(__builtin_strlen)"));
    assert!(truth("!__has_declspec_attribute(noinline)"));
    let (w, d) =
        pp_on(ARM, "#if __is_target_arch(arm64) && !__is_target_arch(x86_64)\narm\n#endif\n", &[]);
    assert_eq!(d, [] as [String; 0]);
    assert_eq!(w, ["arm"]);
}

/// [cpp.cond]/13: `__has_warning` names a diagnostic group.
#[test]
fn has_warning() {
    assert!(truth("__has_warning(\"-Wundef\") && !__has_warning(\"-Wno-such-warning\")"));
}

/// [cpp.embed.param.limit]/1, [cpp.cond]: `#embed` and `__has_embed`
/// parameters are evaluated with `IfEvaluator::eval_value` (the full
/// grammar: macros, `?:`, unevaluated operands). `defined` is not allowed
/// anywhere in the expression; clang 19 only rejects it as the leading
/// operand (deviation, spec/pp.md).
#[test]
fn embed_parameters_use_eval_value() {
    let r = || MemoryResolver::new().with_resource("d.bin", b"abcd");
    let src = "#define N 1\n#embed \"d.bin\" limit(N ? N * 3 - 1 : 1 / 0)\n";
    let (w, d) = pp_with(X86, src, &[], r());
    assert_eq!(w.concat(), "97,98", "{d:?}");
    assert_eq!(d, ["2:2: warning: #embed is a Clang extension [-Wc23-extensions]"]);
    let src = "#if __has_embed(\"d.bin\" limit(0 ? 1/0 : 0)) == 2\nempty\n#endif\n";
    let (w, d) = pp_with(X86, src, &[], r());
    assert_eq!(w, ["empty"]);
    assert_eq!(d, [] as [String; 0], "{d:?}");
    let src = "#define N 1\n#embed \"d.bin\" limit(N + defined(N))\n";
    let (_, d) = pp_with(X86, src, &[], r());
    assert_eq!(d[1], "2:37: error: 'defined' cannot appear within this context");
}
