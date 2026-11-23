use super::clang::{Bare, CLoc};
use super::json::{Event, Reader};
use super::*;

fn s(x: &str) -> String {
    x.to_owned()
}

#[test]
fn json_events_in_order() {
    let mut r =
        Reader::new(&br#" {"a": [1, -2, "x\n\u00e9"], "b": {}, "c": true, "d": null} "#[..]);
    let mut got = Vec::new();
    loop {
        let e = r.next().unwrap();
        if e == Event::End {
            break;
        }
        got.push(e);
    }
    assert_eq!(
        got,
        [
            Event::Obj,
            Event::Key(s("a")),
            Event::Arr,
            Event::Num(Some(1)),
            Event::Num(Some(-2)),
            Event::Str(s("x\né")),
            Event::EndArr,
            Event::Key(s("b")),
            Event::Obj,
            Event::EndObj,
            Event::Key(s("c")),
            Event::Bool(true),
            Event::Key(s("d")),
            Event::Null,
            Event::EndObj,
        ]
    );
}

#[test]
fn json_rejects_bad_documents() {
    for bad in [&b"{\"a\" 1}"[..], b"[1 2]", b"{\"a\": 1", b"[1,]x"] {
        let mut r = Reader::new(bad);
        let mut failed = false;
        for _ in 0..10 {
            match r.next() {
                Err(_) => {
                    failed = true;
                    break;
                }
                Ok(Event::End) => break,
                Ok(_) => {}
            }
        }
        assert!(failed, "{}", String::from_utf8_lossy(bad));
    }
}

fn is_expr(k: &str) -> bool {
    k.ends_with("Expr") || k.ends_with("Literal") || k.ends_with("Operator")
}

/// A dump in clang's shape: the file is printed once and carried forward,
/// through a skipped implicit declaration too; a template's second
/// templated declaration (a specialization) is skipped; a macro location
/// has spelling and expansion locations.
#[test]
fn clang_reader_carries_files_and_skips_specializations() {
    let dump = r#"{"kind":"TranslationUnitDecl","inner":[
      {"kind":"TypedefDecl","isImplicit":true,"loc":{},"range":{"begin":{},"end":{}},
       "inner":[{"kind":"IntegerLiteral","range":{"begin":{"offset":1,"file":"<built-in>","col":1,"tokLen":1},"end":{"offset":1,"col":1,"tokLen":1}}}]},
      {"kind":"FunctionDecl","loc":{"offset":4,"file":"a.cpp","line":1,"col":5,"tokLen":1},
       "range":{"begin":{"offset":0,"col":1,"tokLen":3},"end":{"offset":20,"col":21,"tokLen":1}},
       "inner":[{"kind":"CompoundStmt","range":{"begin":{"offset":8,"col":9,"tokLen":1},"end":{"offset":20,"col":21,"tokLen":1}},
         "inner":[{"kind":"BinaryOperator","range":{"begin":{"offset":10,"col":11,"tokLen":1},"end":{"offset":14,"col":15,"tokLen":2}}}]}]},
      {"kind":"FunctionTemplateDecl","loc":{"offset":40,"file":"b.h","line":1,"col":5,"tokLen":1},
       "inner":[
        {"kind":"TemplateTypeParmDecl","loc":{"offset":41,"col":1,"tokLen":1}},
        {"kind":"FunctionDecl","inner":[{"kind":"CallExpr","range":{"begin":{"offset":50,"col":1,"tokLen":1},"end":{"offset":53,"col":1,"tokLen":1}}}]},
        {"kind":"FunctionDecl","inner":[{"kind":"CallExpr","range":{"begin":{"offset":60,"file":"c.h","col":1,"tokLen":1},"end":{"offset":63,"col":1,"tokLen":1}}}]}]},
      {"kind":"VarDecl","inner":[{"kind":"ParenExpr","range":{
        "begin":{"spellingLoc":{"offset":70,"col":1,"tokLen":1},"expansionLoc":{"offset":90,"col":1,"tokLen":1,"isMacroArgExpansion":true}},
        "end":{"offset":95,"col":1,"tokLen":1}}}]}
    ]}"#;
    let d = clang::read(dump.as_bytes(), &is_expr).unwrap();
    let names: Vec<&str> = d.nodes.iter().map(|n| n.kind.as_str()).collect();
    assert_eq!(names, ["BinaryOperator", "CallExpr", "ParenExpr"]);
    let file = |n: &clang::Node| match n.begin {
        Some(CLoc::File(b)) => d.files[b.file as usize].clone(),
        Some(CLoc::Macro { expansion: Some(x), .. }) => d.files[x.file as usize].clone(),
        _ => String::new(),
    };
    assert_eq!(file(&d.nodes[0]), "a.cpp");
    assert_eq!(d.nodes[0].end, Some(CLoc::File(Bare { file: 1, offset: 14, tok_len: 2 })));
    assert_eq!(file(&d.nodes[1]), "b.h");
    // The skipped specialization's `c.h` still carries forward.
    assert_eq!(file(&d.nodes[2]), "c.h");
    assert!(matches!(d.nodes[2].begin, Some(CLoc::Macro { arg: true, .. })));
}

#[test]
fn kinds_table_parses_and_checks() {
    let k =
        Kinds::parse("# c\nmap A B,C\nnone D nothing written\nskip E why\n  int x;\n  int y;\n\n")
            .unwrap();
    assert!(k.matches("A", "B") && k.matches("A", "C") && k.matches("A", "A"));
    assert!(!k.matches("B", "A") && !k.matches("Z", "B"));
    assert!(k.is_clang_expr("C") && k.is_clang_expr("FooExpr") && !k.is_clang_expr("Stmt"));
    assert!(k.none.contains_key("D") && k.skip.contains_key("E"));
    let k = Kinds::parse("map W *\nquirk d/a.cpp:12 clang's end\n").unwrap();
    assert!(k.matches("W", "Anything") && k.quirk.contains_key(&(s("d/a.cpp"), 12)));
    for bad in [
        "map A",
        "none D",
        "skip E why\n",
        "skip E why\n  1\n  2\n  3\n  4\n",
        "map A B,B",
        "quirk a.cpp why",
        "quirk a.cpp:1",
        "frob A",
        "  stray",
    ] {
        assert!(Kinds::parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn baseline_round_trips() {
    let mut b = Baseline::default();
    let mut kinds = BTreeMap::new();
    kinds.insert(s("CallExpr"), (2, 7));
    b.units.insert(s("tests/a.cpp"), (0xabc, kinds));
    b.units.insert(s("tests/b.cpp"), (7, BTreeMap::new()));
    let text = b.render();
    assert!(text.contains("unit tests/a.cpp 0000000000000abc CallExpr=2/7\n"));
    assert_eq!(Baseline::parse(&text).unwrap(), b);
    assert!(Baseline::parse("unit x zz").is_err());
    assert!(Baseline::parse("unit x 1 A=1").is_err());
    assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
}

#[test]
fn compdb_commands() {
    assert_eq!(
        split_command(r#"/usr/bin/c++ -DX="a b" -I'x y' a\ b -c f.cpp"#),
        ["/usr/bin/c++", "-DX=a b", "-Ix y", "a b", "-c", "f.cpp"]
    );
    let words: Vec<String> =
        split_command("c++ -O2 -std=c++17 -o out.o -c /src/f.cpp -Ifoo -oother.o");
    assert_eq!(compdb_args(&words, "/src/f.cpp"), ["-O2", "-std=c++17", "-Ifoo"]);
}

#[test]
fn kiln_dump_lines() {
    let l = parse_kiln("CallExpr\ta.cpp\t3\t9\nDeclRefExpr\ta.cpp\t4\t-\nCXXDefaultArgExpr\t-\t-\t-\nX\t!\tunmapped\t-\n")
        .unwrap();
    assert_eq!(l[0].extent, Some((s("a.cpp"), 3, 9)));
    assert_eq!(l[1].extent, None);
    assert_eq!(l[2].extent, None);
    assert!(l[3].unmapped && !l[2].unmapped);
    assert!(parse_kiln("CallExpr\ta.cpp\t3\n").is_err());
    assert!(parse_kiln("CallExpr\ta.cpp\tx\t9\n").is_err());
}

#[test]
fn invocation_end_candidates() {
    let t = b"F (a, \")\", g(b)) + G x";
    assert_eq!(invocation_ends(t, 1), [1, 16]);
    assert_eq!(invocation_ends(t, 20), [20]);
}

#[test]
fn invocation_begin_of_a_builtin_macro_result() {
    let t = b"x = __has_builtin (f(1)) + 1";
    assert_eq!(invocation_begin(t, 23), Some(4));
    assert_eq!(invocation_begin(t, 22), Some(19));
    assert_eq!(invocation_begin(t, 0), None);
    assert_eq!(invocation_begin(b"(a)", 2), None);
}

fn node(kind: &str, file_text: bool, ranges: &[(u32, u32)]) -> ClangNode {
    ClangNode {
        kind: s(kind),
        file_text,
        ranges: ranges.iter().map(|&(b, e)| (s("a.cpp"), b, e)).collect(),
        lost_begin: None,
        nested_arg: false,
    }
}

fn line(kind: &str, r: Option<(u32, u32)>) -> KilnLine {
    KilnLine {
        kind: s(kind),
        extent: r.map(|(b, e)| (s("a.cpp"), b, e)),
        unmapped: false,
        at: None,
    }
}

#[test]
fn comparison_matches_multisets_and_macros() {
    let kinds = Kinds::parse("none CXXDefaultArgExpr x\nskip Odd y\n  int z;\n").unwrap();
    let clang = [
        node("CallExpr", true, &[(0, 5)]),
        node("ImplicitCastExpr", true, &[(0, 1)]),
        node("ImplicitCastExpr", true, &[(0, 1)]),
        node("ParenExpr", false, &[(10, 14), (10, 20)]),
        node("DeclRefExpr", true, &[(30, 31)]),
        node("Odd", true, &[(40, 41)]),
    ];
    let kiln = [
        line("CallExpr", Some((0, 5))),
        line("ImplicitCastExpr", Some((0, 1))),
        line("ImplicitCastExpr", Some((0, 1))),
        line("ParenExpr", Some((10, 20))),
        line("DeclRefExpr", None),
        line("Odd", Some((40, 50))),
    ];
    let text = |_: &str, _: u32, _: u32| String::new();
    let c = compare(&kinds, &kiln, &clang, 2, &text);
    assert!(c.mismatches.is_empty(), "{:?}", c.mismatches);
    assert_eq!(c.compared, 4);
    assert_eq!(c.macro_skipped, 2);
    assert_eq!(c.kinds["DeclRefExpr"], KindCounts { some: 0, none: 1, clang_only: 1, mismatch: 0 });
    assert_eq!(c.kinds["ImplicitCastExpr"].some, 2);

    // A third ImplicitCastExpr (ends are sets: not a mismatch), a wrong
    // end, an extent clang has no node for, an extent for a `none` kind,
    // and a kind that matches clang's by a `map` line.
    let kiln = [
        line("ImplicitCastExpr", Some((0, 1))),
        line("ImplicitCastExpr", Some((0, 1))),
        line("ImplicitCastExpr", Some((0, 1))),
        line("CallExpr", Some((0, 4))),
        line("MemberExpr", Some((2, 3))),
        line("CXXDefaultArgExpr", Some((9, 10))),
        line("BinaryOperator", Some((30, 31))),
    ];
    let kinds = Kinds::parse("none CXXDefaultArgExpr x\nmap BinaryOperator DeclRefExpr\n").unwrap();
    let c = compare(&kinds, &kiln, &clang, 0, &text);
    assert_eq!(c.mismatches.len(), 3, "{:?}", c.mismatches);
    assert!(c.mismatches.iter().any(|m| m.starts_with("a.cpp:0-4 CallExpr kiln \"\" clang 0-5")));
    assert!(c.mismatches.iter().any(|m| m.contains("MemberExpr") && m.contains("clang - ")));
    assert_eq!(c.kinds["CallExpr"].mismatch, 1);
}

/// The places clang gives no comparable node: a wrapper whose range a
/// matched node has, text clang compiled no expression of, a node clang
/// lost the begin of, and a `quirk`.
#[test]
fn comparison_passes_what_clang_cannot_show() {
    let kinds = Kinds::parse("quirk a.cpp:20 clang's end\n").unwrap();
    let mut lost = node("CXXUnresolvedConstructExpr", false, &[]);
    lost.lost_begin = Some((s("a.cpp"), 30));
    let clang = [
        node("CallExpr", true, &[(0, 5)]),
        node("InitListExpr", true, &[(28, 30)]),
        lost,
        node("ParenExpr", true, &[(20, 26)]),
    ];
    let kiln = [
        line("CallExpr", Some((0, 5))),
        line("CXXBindTemporaryExpr", Some((0, 5))),
        line("IntegerLiteral", Some((40, 41))),
        line("CXXUnresolvedConstructExpr", Some((24, 30))),
        line("ParenExpr", Some((20, 27))),
    ];
    let text = |_: &str, _: u32, _: u32| String::new();
    let c = compare(&kinds, &kiln, &clang, 0, &text);
    assert!(c.mismatches.is_empty(), "{:?}", c.mismatches);
    // The node of text clang did not compile is not compared, the quirk
    // place is skipped.
    assert_eq!(c.compared, 3);
    // The same wrong end without the quirk is a mismatch.
    let c = compare(&Kinds::default(), &kiln, &clang, 0, &text);
    assert_eq!(c.mismatches.len(), 1, "{:?}", c.mismatches);
}

#[test]
fn stage_one_units_exclude_what_cannot_be_compared() {
    let spec = |src: &str| directives::parse(src, Lang::Cxx).unwrap();
    let ok = "//@ expect: compile-ok\nint x;\n";
    assert_eq!(not_a_unit(ok, &spec(ok)), None);
    let fail = "//@ expect: compile-fail\nint x = ; //@ diag: error\n";
    assert!(not_a_unit(fail, &spec(fail)).is_some());
    let inc = "//@ expect: compile-ok\n#include <vector>\n";
    assert!(not_a_unit(inc, &spec(inc)).is_some());
    let local = "//@ expect: compile-ok\n#include \"Inputs/a.h\"\n";
    assert_eq!(not_a_unit(local, &spec(local)), None);
    let kiln_only = "//@ expect: compile-ok\n//@ args: -fkiln-foo\nint x;\n";
    assert!(not_a_unit(kiln_only, &spec(kiln_only)).is_some());
}

/// spec/ast.md §21.3 tables exactly the kinds the baseline has a `None`
/// of.
#[test]
fn the_none_table_is_the_baselines_kinds() {
    let root = crate::util::repo_root();
    let base = std::fs::read_to_string(root.join(BASELINE)).unwrap();
    let base = Baseline::parse(&base).unwrap();
    let want: std::collections::BTreeSet<&str> = base
        .units
        .values()
        .flat_map(|(_, kinds)| kinds.iter().filter(|(_, (n, _))| *n > 0).map(|(k, _)| k.as_str()))
        .collect();
    let spec = std::fs::read_to_string(root.join("spec/ast.md")).unwrap();
    let section = spec
        .split("### 21.3 ")
        .nth(1)
        .and_then(|s| s.split("\n### ").next())
        .expect("spec/ast.md has §21.3");
    let mut have = std::collections::BTreeSet::new();
    for row in section.lines().filter(|l| l.starts_with("| `")) {
        let kinds = row.split('|').nth(1).unwrap_or("");
        for k in kinds.split(',') {
            have.insert(k.trim().trim_matches('`').to_owned());
        }
    }
    let have: std::collections::BTreeSet<&str> = have.iter().map(String::as_str).collect();
    assert_eq!(have, want, "spec/ast.md §21.3 against {BASELINE}");
}
