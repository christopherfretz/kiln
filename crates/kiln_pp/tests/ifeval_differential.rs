//! [cpp.cond] differential test: `#if` expressions evaluated by kiln's
//! [`kiln_pp::IfEval`] and by `clang++-19 -E` must select the same groups
//! and produce the same diagnostics (location, level, wording, option).
//!
//! Every case runs for aarch64 and x86-64 Linux (the signedness of `char`
//! and `wchar_t` differs), with default diagnostics and with
//! `-pedantic -Wundef -Wfour-char-constants`. Skipped if `clang++-19` is
//! not installed.

mod ifeval_common;

use std::fmt::Write as _;

use ifeval_common::{clang_available, run_clang, run_kiln, words};
use kiln_lex::Std;
use kiln_pp::MemoryResolver;

/// The cases, by standard. A case is an expression, optionally preceded
/// by `#define` lines (one per line); the macros are `#undef`ined after.
const CASES: &[(Std, &[&str])] = &[
    (
        Std::Cxx26,
        &[
            // [cpp.cond]/12: arithmetic in intmax_t / uintmax_t.
            "1",
            "0",
            "1 + 2 * 3 == 7",
            "(1 + 2) * 3 == 9",
            "10 / 3 == 3",
            "-10 / 3 == -3",
            "-10 % 3 == -1",
            "10 % -3 == 1",
            "7 - 3 - 2 == 2",
            "2 * 3 % 4 == 2",
            "1 << 2 << 3 == 32",
            "256 >> 2 >> 1 == 32",
            "1 < 2 == 1",
            "1 == 1 == 1",
            "3 & 5 == 1",
            "1 | 2 ^ 3 & 4",
            "0 || 0 && 1",
            "1 || 0 && 0",
            "~0 == -1",
            "!0 == 1",
            "!!5",
            "-(-3) == 3",
            "+3 == 3",
            "~0u == 18446744073709551615u",
            "0x7fffffffffffffff + 1",
            "-0x7fffffffffffffff - 1 < 0",
            "-9223372036854775807 - 1 < 0",
            "9223372036854775807 + 1",
            "-9223372036854775807 - 2",
            "(-9223372036854775807-1) / -1",
            "(-9223372036854775807-1) % -1 == 0",
            "-(-9223372036854775807-1)",
            "4294967296 * 4294967296",
            "3037000500 * 3037000500",
            "-1 * -1 == 1",
            "1u - 2 > 0",
            "0u - 1 == 18446744073709551615",
            "-1 < 0u",
            "0u > -1",
            "-1 == 18446744073709551615u",
            "-1 > 0u ? 1 : 0",
            "1 ? 0u : -1",
            "(1 ? -1 : 0u) > 0",
            "(0 ? 0u : -1) > 0",
            "-1 / 2u",
            "-1 % 2u",
            "-2 * 1u",
            "-1 & 1u",
            "(-1 | 0u) == 18446744073709551615u",
            "-1 + -1u",
            "-1 >> 1 == -1",
            "-1 >> 63 == -1",
            "-1 >> 64",
            "1 >> 64",
            "1 >> -1",
            "1u >> 63 == 0",
            "18446744073709551615u >> 63 == 1",
            "1 << 62",
            "1 << 63",
            "1 << 64",
            "1 << -1",
            "-1 << 1 == -2",
            "-1 << 63",
            "-1 << 64",
            "-4 << 61",
            "-5 << 61",
            "1u << 63",
            "1u << 64",
            "2u << 63",
            "1u << -1",
            "0 << 64",
            "0 << 63",
            "3 << 62",
            "0 && (1 << 64)",
            "-1u",
            "-0x8000000000000000",
            "-9223372036854775808",
            "9223372036854775808",
            "18446744073709551615",
            "18446744073709551616",
            "0x10000000000000000",
            "0xffffffffffffffff",
            "0xffffffffffffffff == -1",
            "01777777777777777777777",
            "0b1111111111111111111111111111111111111111111111111111111111111111",
            "0 && 18446744073709551616",
            "0 && 9223372036854775808",
            // Liveness ([cpp.cond]/12, [expr.log.and], [expr.cond]).
            "0 && 1/0",
            "1 || 1/0",
            "0 ? 1/0 : 2",
            "1 ? 2 : 1/0",
            "1/0",
            "1%0",
            "1/(1-1)",
            "0 && (1/0)",
            "(0 && 1) ? 4 : 1/0",
            "0 && 1 ? 4 : 1/0",
            "1 || (0 ? 1 : 1/0)",
            "0 ? 1 : 2 ? 3 : 4",
            "1 ? 2 : 3 ? 4 : 5",
            "1 ? 2 ? 3 : 4 : 5",
            "0 ? 2 ? 3 : 4 : 5",
            // Comma and colon.
            "0 ? (1, 2) : 3",
            "1, 2",
            "(1, 2)",
            "(0, 1)",
            "(1, 0)",
            "0 && (1, 0)",
            "(1 ? 2, 3 : 4)",
            "1 ? 2 : 3, 4",
            "(1 ? 2 : 3 : 4)",
            "1 : 2",
            "(1 : 2)",
            // Syntax errors.
            "1 ?",
            "1 ? 2",
            "1 ? 2 :",
            "1 ? : 2",
            "(1 ? 2)",
            "(1",
            "(1 + 2",
            "((1)",
            "1)",
            "()",
            "(",
            "",
            "+",
            "1 +",
            "1 2",
            "1 (2)",
            "1 ? 2 3",
            "1 = 1",
            "1 += 1",
            "a = 1",
            "++1",
            "1++",
            "\"abc\"",
            "1 \"abc\"",
            "@",
            "1 @",
            "1 <=> 2",
            "[1]",
            "1 [",
            "1 ; 2",
            // Literals ([lex.icon], [lex.fcon]).
            "1.0",
            "1e3",
            "0x1p3",
            "1.",
            ".5",
            "1i",
            "1il",
            "1ui",
            "1if",
            "1_km",
            "1.0_km",
            "1'000'000 == 1000000",
            "1'000'",
            "0b102",
            "08",
            "09.5",
            "0x",
            "0x.p1",
            "0x1.8",
            "1e",
            "1q",
            "1uu",
            "1lul",
            "1llu == 1",
            "1LL",
            "1z",
            "1uz",
            "1zu",
            "1ull",
            "0X1F == 31",
            "017 == 15",
            "0b101 == 5",
            "0B11 == 3",
            "1wb",
            "1fi",
            "1ii",
            "1if_",
            "1h",
            "1min",
            "1d",
            "1fl",
            "1.0if",
            "1e1i",
            "1zi",
            "1jl",
            // Identifiers ([cpp.cond]/13).
            "true",
            "false",
            "true + true == 2",
            "true ? 1 : 0",
            "undefined_x",
            "undefined_x + 1",
            "0 && undefined_y",
            "1 || undefined_z",
            "0 ? undefined_w : 1",
            // Alternative tokens ([lex.digraph]).
            "and",
            "1 and 0",
            "1 or 0",
            "not 0",
            "compl 0 == -1",
            "1 bitand 3",
            "1 bitor 2 == 3",
            "1 xor 3 == 2",
            "1 not_eq 2",
            "1 and_eq 1",
            "1 bitor",
            // defined ([cpp.cond]/10).
            "defined",
            "defined(",
            "defined X",
            "defined(X)",
            "defined(X",
            "defined(X) + 1",
            "defined 1",
            "defined(1)",
            "defined and",
            "defined(__cplusplus)",
            "defined __cplusplus && __cplusplus >= 201103L",
            "!defined(NOPE)",
            "defined defined",
            "defined(X Y)",
            // Macros.
            "#define ONE 1\nONE + ONE == 2",
            "#define EMPTY\nEMPTY 1",
            "#define EMPTY\n1 EMPTY",
            "#define EMPTY\nEMPTY",
            "#define ID(x) x\nID(1) + ID(2) == 3",
            "#define ONE\n#define D defined(ONE)\nD",
            "#define ONE\n#define DF(x) defined(x)\nDF(ONE)",
            "#define P (\nP 1)",
            "#define Z 0\n1 / Z",
            "#define Z 0\n0 && 1 / Z",
            "#define NEG -1\nNEG < 0u",
            "#define BIG 18446744073709551615\nBIG",
            "#define FL 1.5\nFL",
            "#define OP +\n1 OP 2 == 3",
            "#define Q ?\n1 Q 2 : 3",
            "#define C ,\n(1 C 2)",
            // Character literals ([lex.ccon], [cpp.cond]/12).
            "'a' == 97",
            "'\\n' == 10",
            "'\\0' == 0",
            "'\\x41' == 65",
            "'\\101' == 65",
            "'\\377' < 0",
            "'\\xff' > 0",
            "'\\xff' == 255",
            "'\\x80' < 0u",
            "L'\\xffffffff' < 0",
            "u'\\xffff' > 0",
            "U'\\xffffffff' > 0",
            "u8'a' == 97",
            "u8'\\xff' > 0",
            "'ab' == 24930",
            "'abcd' == 1633837924",
            "'abcde'",
            "L'ab'",
            "u'ab'",
            "U'ab'",
            "u8'ab'",
            "'\\q'",
            "'\\e' == 27",
            "'\\x'",
            "'\\x{41}' == 65",
            "'\\o{101}' == 65",
            "'\\x{}'",
            "'\\x{41'",
            "'\\x{4g}'",
            "'\\o'",
            "'\\x100'",
            "'\\400'",
            "'\\u0041' == 65",
            "u'\\u00e9' == 233",
            "U'\\U0001F600' == 128512",
            "'\\N{LATIN SMALL LETTER A}' == 97",
            "'\\N{latin small letter a}'",
            "'\\N{NOPE}'",
            "'\\u12'",
            "'\u{e9}'",
            "'a'_x",
            "'\\''",
            "'\"' == 34",
            "L'a' == 97",
            "'\\a' + '\\b' + '\\f' + '\\r' + '\\t' + '\\v' == 52",
        ],
    ),
    (Std::Cxx98, &["1LL", "1ll == 1", "0x1p3", "0b11", "1i", "'\\u0041'"]),
    (Std::Cxx11, &["0b101 == 5", "1i", "1_x", "1LL", "1z"]),
    (Std::Cxx14, &["0x1p3", "0x1.8p1", "u8'a'", "'\\x{41}'", "1'0 == 10"]),
    (Std::Cxx17, &["u8'\\xff' > 0", "u8'a' == 97", "0x1p3", "1z"]),
    (Std::Cxx20, &["u8'\\xff' > 0", "'\\N{DIGIT ONE}' == 49", "1z", "1 <=> 2"]),
];

/// Cases whose value differs for reasons outside the evaluator, with the
/// reason; their diagnostics are still compared.
const KNOWN_VALUE_DIFFERENCES: &[(&str, &str)] = &[(
    "'\\x{41'",
    "kiln_lex::literal::decode_char gives 0 for an unterminated delimited escape; \
     clang keeps the digits read (0x41)",
)];

const TRIPLES: &[&str] = &["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"];
const FLAGS: &[&[&str]] = &[&[], &["-pedantic", "-Wundef", "-Wfour-char-constants"]];

/// The source for `cases` and, per case, its `#if` line range.
fn build(cases: &[&str]) -> (String, Vec<(u32, u32)>) {
    let mut src = String::new();
    let mut ranges = Vec::new();
    let mut line = 1u32;
    for (i, case) in cases.iter().enumerate() {
        let start = line;
        let mut lines: Vec<&str> = case.split('\n').collect();
        let expr = lines.pop().unwrap_or_default();
        for l in &lines {
            src.push_str(l);
            src.push('\n');
        }
        let _ = write!(src, "#if {expr}\nc{i}t\n#else\nc{i}f\n#endif\n");
        line += u32::try_from(lines.len()).unwrap() + 5;
        for l in &lines {
            if let Some(name) = l.strip_prefix("#define ") {
                let name = name.split([' ', '(']).next().unwrap_or_default();
                let _ = writeln!(src, "#undef {name}");
                line += 1;
            }
        }
        ranges.push((start, line - 1));
    }
    (src, ranges)
}

fn line_of(d: &str) -> u32 {
    d.split(':').next().and_then(|l| l.parse().ok()).unwrap_or(0)
}

#[test]
fn differential_vs_clang() {
    if !clang_available() {
        eprintln!("skipped: clang++-19 not installed");
        return;
    }
    let mut failures = Vec::new();
    let mut total = 0usize;
    for &(std, cases) in CASES {
        let (src, ranges) = build(cases);
        for triple in TRIPLES {
            for flags in FLAGS {
                let Some(want) = run_clang(&src, triple, std, flags) else {
                    panic!("could not run clang");
                };
                let got = run_kiln(&src, triple, std, flags, MemoryResolver::new(), |_| {});
                let (gw, ww) = (words(&got.text), words(&want.text));
                for (i, case) in cases.iter().enumerate() {
                    total += 1;
                    let (lo, hi) = ranges[i];
                    let pick = |v: &[String]| -> Vec<String> {
                        v.iter().filter(|d| (lo..=hi).contains(&line_of(d))).cloned().collect()
                    };
                    let (gd, wd) = (pick(&got.diags), pick(&want.diags));
                    let t = format!("c{i}t");
                    let f = format!("c{i}f");
                    let sel = |w: &[&str]| {
                        w.iter().find(|x| **x == t || **x == f).map(|x| (*x).to_owned())
                    };
                    let known = KNOWN_VALUE_DIFFERENCES.iter().any(|(c, _)| c == case);
                    if gd != wd || (sel(&gw) != sel(&ww) && !known) {
                        failures.push(format!(
                            "[{std:?} {triple} {flags:?}] #if {case:?}\n  kiln:  {:?} {gd:#?}\n  clang: {:?} {wd:#?}",
                            sel(&gw),
                            sel(&ww)
                        ));
                    }
                }
                // Diagnostics outside any case (none expected).
                let stray = |v: &[String]| -> Vec<String> {
                    v.iter()
                        .filter(|d| !ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&line_of(d))))
                        .cloned()
                        .collect()
                };
                assert_eq!(stray(&got.diags), stray(&want.diags), "[{std:?} {triple} {flags:?}]");
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {total} case runs differ from clang:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
