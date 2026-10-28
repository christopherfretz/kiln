//! Unit tests of the normalizer. The end-to-end fixtures under
//! `xtask/fixtures/ast/normalize/` are clang 19 dumps (`<name>.json`, the
//! compacted `-ast-dump=json`; `<name>.txt`, the text dump) with the
//! normalized output `<name>.expected`; they run without clang.

use std::path::PathBuf;

use super::side::{self, VNode};
use super::table::{self, Disposition, M2_KINDS, TABLE};
use super::values::{self, Field, Literals, Record, Shape, Types, Value};
use super::{DumpOptions, escape, normalize};

fn fixture(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/ast/normalize").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Normalizes fixture `name` and counts the side-channel fetches.
fn run_fixture(name: &str, opts: &DumpOptions) -> (Result<String, String>, usize) {
    let json = fixture(&format!("{name}.json"));
    let side = fixture(&format!("{name}.txt"));
    let mut calls = 0;
    let mut text = || {
        calls += 1;
        Ok(side.clone())
    };
    let r = normalize(&json, &format!("{name}.cpp"), opts, &mut text);
    (r, calls)
}

#[test]
fn spec_example_reproduces() {
    let (r, calls) = run_fixture("ex", &DumpOptions::default());
    assert_eq!(r.unwrap(), fixture("ex.expected"));
    assert_eq!(calls, 0, "no constexpr variable: the text dump is not needed");
    let f = DumpOptions { filter: Some("x".into()), ..DumpOptions::default() };
    assert_eq!(run_fixture("ex", &f).0.unwrap(), fixture("ex.filter-x.expected"));
    // The main file is matched by name: another name prints no declarations.
    let json = fixture("ex.json");
    let mut text = || Err("unused".to_owned());
    let r = normalize(&json, "other.cpp", &DumpOptions::default(), &mut text).unwrap();
    assert_eq!(r, "TranslationUnitDecl\n");
}

#[test]
fn constexpr_values_come_from_the_text_dump() {
    let (r, calls) = run_fixture("values", &DumpOptions::default());
    assert_eq!(r.unwrap(), fixture("values.expected"));
    assert_eq!(calls, 1, "the text dump is fetched once");
    // A text dump that does not align with the JSON is an error, not a guess.
    let json = fixture("values.json");
    let side = fixture("values.txt").replacen(" o 'const O'", " p 'const O'", 1);
    let mut text = || Ok(side.clone());
    let e = normalize(&json, "values.cpp", &DumpOptions::default(), &mut text).unwrap_err();
    assert!(e.contains("does not match JSON VarDecl `o`"), "{e}");
    let mut fails = || Err("boom".to_owned());
    let e = normalize(&json, "values.cpp", &DumpOptions::default(), &mut fails).unwrap_err();
    assert!(e.starts_with("boom"), "{e}");
}

/// A translation unit holding `decl` (a JSON object) at `t.cpp:1:1`.
fn tu(decl: &str) -> String {
    format!(
        r#"{{"id":"0x1","kind":"TranslationUnitDecl","loc":{{}},"range":{{}},"inner":[{decl}]}}"#
    )
}

fn norm(json: &str) -> Result<String, String> {
    let mut text = || Err("no text dump in this test".to_owned());
    normalize(json, "t.cpp", &DumpOptions::default(), &mut text)
}

const LOC: &str = r#""loc":{"offset":0,"file":"t.cpp","line":1,"col":1,"tokLen":1}"#;

#[test]
fn unmapped_input_is_an_error_naming_it() {
    let var = |extra: &str, inner: &str| {
        tu(&format!(
            r#"{{"id":"0x2","kind":"VarDecl",{LOC},"range":{{}},"name":"v","type":{{"qualType":"int"}}{extra},"inner":[{inner}]}}"#
        ))
    };
    let lit = r#"{"id":"0x3","kind":"IntegerLiteral","range":{},"type":{"qualType":"int"},"valueCategory":"prvalue","value":"1"}"#;
    assert_eq!(
        norm(&var(r#","init":"c""#, lit)).unwrap(),
        "TranslationUnitDecl\n`-VarDecl v 'int' cinit\n  `-IntegerLiteral 'int' 1\n"
    );
    let e = norm(&var("", &lit.replace("IntegerLiteral", "FrobExpr"))).unwrap_err();
    assert!(e.contains("clang node kind `FrobExpr` is not in the clang-ast mapping table"), "{e}");
    assert!(e.contains("in top-level VarDecl `v`"), "{e}");
    let e = norm(&var("", &lit.replace("IntegerLiteral", "CXXConstructExpr"))).unwrap_err();
    assert!(e.contains("unsupported CXXConstructExpr"), "{e}");
    let e = norm(&var(r#","tls":"static""#, lit)).unwrap_err();
    assert!(e.contains("`VarDecl` attribute `tls` is not in the mapping table"), "{e}");
    let e = norm(&var(r#","init":"parenlist""#, lit)).unwrap_err();
    assert!(e.contains("unsupported init style"), "{e}");
    let e =
        norm(&tu(r#"{"id":"0x2","kind":"VarDecl","loc":{},"range":{},"name":"v"}"#)).unwrap_err();
    assert!(e.contains("has no source location"), "{e}");
    assert!(norm("[]").unwrap_err().contains("not a TranslationUnitDecl"));
    assert!(norm("{").unwrap_err().contains("clang's JSON dump"));
}

#[test]
fn mapping_table_covers_m2() {
    for k in M2_KINDS {
        assert!(matches!(table::lookup(k), Ok(Disposition::Keep)), "{k} must be kept");
        // Every kept kind has an attribute list.
        table::check_keys(k, ["id", "kind"].iter()).unwrap();
    }
    for (k, d) in TABLE {
        if matches!(d, Disposition::Keep) {
            assert!(M2_KINDS.contains(k), "{k} is kept but not an M2 kind");
        }
        assert_eq!(TABLE.iter().filter(|(x, _)| x == k).count(), 1, "{k} listed twice");
    }
    let e = table::lookup("NoSuchExpr").unwrap_err();
    assert!(e.contains("`NoSuchExpr`") && e.contains("table.rs"), "{e}");
    assert!(matches!(table::lookup("AlignedAttr"), Ok(Disposition::Unsupported(_))));
    assert!(matches!(table::lookup("FullComment"), Ok(Disposition::Drop)));
    let e = table::check_keys("IntegerLiteral", ["value", "isWeird"].iter()).unwrap_err();
    assert!(e.contains("`isWeird`"), "{e}");
    // Decl keys are not expression keys.
    assert!(table::check_keys("IntegerLiteral", ["isUsed"].iter()).is_err());
    let (keep, drop, unsupported) = table::counts();
    assert_eq!(keep, M2_KINDS.len());
    assert_eq!(keep + drop + unsupported, TABLE.len());
    eprintln!("mapping table: {keep} keep, {drop} drop, {unsupported} unsupported");
}

#[test]
fn string_literals_are_respelled() {
    let r = |s: &str| escape::respell(s).unwrap();
    assert_eq!(r(r#""msg""#), r#""msg""#);
    assert_eq!(r(r#""a\\b\"c\n\t\r\a\b\f\v\000""#), r#""a\\b\"c\n\t\r\a\b\f\v\0""#);
    // Octal and hex escapes, and the `""` splice clang prints after `\x`.
    assert_eq!(r(r#""\377\001x""#), r#""\x{ff}\x{1}x""#);
    assert_eq!(r(r#""\xFF""A""#), r#""\x{ff}A""#);
    assert_eq!(r(r#""\x7F~""#), r#""\x{7f}~""#);
    // clang prints bytes and small code units in octal, others as `\U`.
    assert_eq!(r(r#"u8"\303\251""#), r#"u8"\x{c3}\x{a9}""#);
    assert_eq!(r(r#"u"a\351\U0001F600""#), r#"u"a\x{e9}\x{d83d}\x{de00}""#);
    assert_eq!(r(r#"U"\U0001F600""#), r#"U"\x{1f600}""#);
    assert_eq!(r(r#"L"w""#), r#"L"w""#);
    assert!(escape::respell("\"\u{e9}\"").is_err(), "raw non-ASCII is not clang's output");
    assert!(escape::respell("x").is_err());
}

#[test]
fn text_dump_side_channel() {
    let text = "\
TranslationUnitDecl 0x1 <<invalid sloc>> <invalid sloc>
|-FunctionDecl 0x2 <t.cpp:1:1, col:30> col:15 constexpr f 'int ()' implicit-inline
| `-CompoundStmt 0x3 <col:19, col:30>
|-VarDecl 0x4 <line:2:1, col:30> col:15 v 'const S' constexpr cinit
| |-value: Struct
| | |-field: Int 1
| | `-fields: Int 2, Float 1.500000e+00
| `-InitListExpr 0x5 <col:19, col:30> 'const S'
`-VarDecl 0x6 <line:3:1, col:5> col:5 w 'int'
";
    let d = side::parse(text).unwrap();
    assert_eq!(d.len(), 3);
    assert_eq!((d[0].class, d[0].names("f")), ("FunctionDecl", true));
    assert!(d[0].has_flag("constexpr") && d[0].has_flag("implicit-inline"));
    assert!(!d[0].has_flag("int") && !d[0].has_flag("consteval"));
    assert!(d[2].value.is_none());
    let v = values::parse(d[1].value.as_ref().unwrap()).unwrap();
    assert_eq!(
        v,
        Value::Struct(vec![
            Value::Int("1".into()),
            Value::Int("2".into()),
            Value::Float("1.500000e+00".into())
        ])
    );
}

fn types() -> Types {
    let mut t = Types::default();
    let f = |n: &str, ty: &str| Field { name: Some(n.into()), ty: ty.into() };
    t.add_record(
        "S",
        Record { union: false, fields: vec![f("a", "int"), f("b", "bool"), f("d", "double")] },
    );
    t.add_record("U", Record { union: true, fields: vec![f("i", "int"), f("s", "S")] });
    t.add_alias("Arr", "int[3]");
    t.enums.insert("E".into());
    t
}

#[test]
fn type_shapes() {
    let t = types();
    let s = |ty: &str| t.shape(ty).unwrap();
    assert_eq!(s("const int"), Shape::Int);
    assert_eq!(s("unsigned long long"), Shape::Int);
    assert_eq!(s("bool"), Shape::Bool);
    assert_eq!(s("const float"), Shape::F32);
    assert_eq!(s("std::nullptr_t"), Shape::NullPtr);
    assert_eq!(s("int *const"), Shape::Pointer);
    assert_eq!(s("const char &"), Shape::Pointer);
    assert_eq!(s("int (*)(int)"), Shape::Pointer);
    assert_eq!(s("const Arr"), Shape::Array { len: 3, elem: "int".into() });
    assert_eq!(s("S[2][3]"), Shape::Array { len: 2, elem: "S[3]".into() });
    assert_eq!(s("struct S"), Shape::Record("S".into()));
    assert_eq!(s("E"), Shape::Int);
    assert!(t.shape("long double").is_err());
    assert!(t.shape("Unknown").is_err());
}

#[test]
#[allow(clippy::many_single_char_names)]
fn values_print_as_kiln_does() {
    let t = types();
    let lits = Literals { values: vec!["1.5".into(), "0.1".into()] };
    let p = |v: &Value, ty: &str| values::print_var(&t, v, ty, &lits, false);
    let int = |s: &str| Value::Int(s.into());
    assert_eq!(p(&int("1"), "bool").unwrap(), "true");
    assert_eq!(p(&int("-7"), "long").unwrap(), "-7");
    assert_eq!(p(&Value::Float("1.500000e+00".into()), "double").unwrap(), "1.5e0");
    assert_eq!(p(&Value::Float("-1.000000e-01".into()), "double").unwrap(), "-1e-1");
    assert_eq!(p(&Value::Float("0.000000e+00".into()), "double").unwrap(), "0e0");
    // Seven digits cannot identify a value no literal spells.
    assert!(p(&Value::Float("3.333333e-01".into()), "double").is_err());
    let s = Value::Struct(vec![int("1"), int("0"), Value::Float("1.500000e+00".into())]);
    assert_eq!(p(&s, "const S").unwrap(), "{1, false, 1.5e0}");
    let a = Value::Array { size: 3, elems: vec![int("4")], filler: Some((2, Box::new(int("0")))) };
    assert_eq!(p(&a, "Arr").unwrap(), "{4, 0, 0}");
    let u = Value::Union(Some(("s".into(), Box::new(s.clone()))));
    assert_eq!(p(&u, "U").unwrap(), "{.s = {1, false, 1.5e0}}");
    assert_eq!(p(&Value::Union(None), "U").unwrap(), "{}");
    assert_eq!(values::print_var(&t, &Value::Address, "int *", &lits, true).unwrap(), "nullptr");
    assert!(p(&Value::Address, "int *").is_err(), "a non-null pointer value is unknown");
    // A value that does not fit its type is an error.
    assert!(p(&s, "int").is_err());
    let long = Value::Array { size: 40, elems: vec![], filler: Some((40, Box::new(int("0")))) };
    let out = p(&long, "int[40]").unwrap();
    assert!(out.ends_with(", 0, ...}") && out.matches('0').count() == 32, "{out}");
    assert_eq!(values::constant(&t, "true", "bool").unwrap(), "true");
    assert_eq!(values::constant(&t, "-3", "E").unwrap(), "-3");
    assert!(values::constant(&t, "1.5", "double").is_err());
    assert!(values::constant(&t, "&x", "int *").is_err());
}

#[test]
fn value_tree_forms() {
    let leaf = |label: &str, text: &str| VNode {
        label: label.into(),
        text: text.into(),
        children: vec![],
    };
    let arr = VNode {
        label: "value".into(),
        text: "Array size=4".into(),
        children: vec![leaf("elements", "Int 1, Int 2"), leaf("filler", "2 x Int 0")],
    };
    assert_eq!(
        values::parse(&arr).unwrap(),
        Value::Array {
            size: 4,
            elems: vec![Value::Int("1".into()), Value::Int("2".into())],
            filler: Some((2, Box::new(Value::Int("0".into()))))
        }
    );
    assert_eq!(
        values::parse(&leaf("value", "Union .i Int 7")).unwrap(),
        Value::Union(Some(("i".into(), Box::new(Value::Int("7".into())))))
    );
    assert!(values::parse(&leaf("value", "ComplexInt 1 + 2i")).is_err());
}
