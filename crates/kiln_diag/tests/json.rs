//! The JSON renderer (`spec/diag.md` section 10).

mod common;

use common::{at, json, mac, source, sp, sub};
use kiln_diag::{Diag, DiagId, FixIt, escape_json};
use kiln_source::Span;

// spec: diag.md §10.2 (string escaping)
#[test]
fn escaping() {
    assert_eq!(escape_json("plain"), "plain");
    assert_eq!(escape_json("a\"b\\c"), "a\\\"b\\\\c");
    assert_eq!(escape_json("\n\r\t\u{8}\u{c}"), "\\n\\r\\t\\b\\f");
    assert_eq!(escape_json("\0\u{1}\u{1f}\u{7f}"), "\\u0000\\u0001\\u001f\\u007f");
    assert_eq!(escape_json("é\u{fffd}€ \u{80}"), "é\u{fffd}€ \u{80}");
    assert_eq!(escape_json("/<>'"), "/<>'");
    // Every char below U+0080 round-trips through the escape.
    for c in (0u8..0x80).map(char::from) {
        let e = escape_json(&c.to_string());
        let back = match e.as_str() {
            "\\\"" => '"',
            "\\\\" => '\\',
            "\\n" => '\n',
            "\\r" => '\r',
            "\\t" => '\t',
            "\\b" => '\u{8}',
            "\\f" => '\u{c}',
            s if s.starts_with("\\u") => {
                char::from_u32(u32::from_str_radix(&s[2..], 16).unwrap()).unwrap()
            }
            s => {
                assert_eq!(s.len(), 1, "{c:?} escaped as {s}");
                s.chars().next().unwrap()
            }
        };
        assert_eq!(back, c);
    }
}

// spec: diag.md §10.1 (object structure, nulls, notes, spans, fixits)
#[test]
fn structure() {
    let src = "#define A x\n#define A \"y\\\"\tz\"\n";
    let (sm, f) = source("dir/j.c", src);
    let s = &sm;
    let lines = json(s, &["-ferror-limit=0", "-Wbogus"], |e| {
        Diag::new(DiagId::pp_macro_redefined, sp(s, f, 2, 9, 1))
            .arg("A")
            .span(sp(s, f, 2, 11, 7), "new \"body\"")
            .range(sp(s, f, 2, 1, 7))
            .note(DiagId::note_previous_definition, sp(s, f, 1, 9, 1))
            // Removes the whole line, newline included: the end is 3:1.
            .fixit(FixIt::remove(sp(s, f, 2, 1, 18)))
            .fixit(FixIt::insert(at(s, f, 1, 12), "\n"))
            .emit(e);
        Diag::new(DiagId::pp_endif_without_if, Span::INVALID).emit(e);
    });
    assert_eq!(lines.len(), 3, "{lines:#?}");
    assert_eq!(
        lines[0],
        r#"{"id":"diag_unknown_warning_option","severity":"warning","file":null,"line":null,"col":null,"message":"unknown warning option '-Wbogus'; did you mean '-Wmost'?","group":"unknown-warning-option","option":"-Wunknown-warning-option","notes":[],"spans":[],"fixits":[]}"#
    );
    assert_eq!(
        lines[1],
        concat!(
            r#"{"id":"pp_macro_redefined","severity":"warning","file":"dir/j.c","line":2,"col":9,"#,
            r#""message":"'A' macro redefined","group":"macro-redefined","option":"-Wmacro-redefined","#,
            r#""notes":[{"id":"note_previous_definition","severity":"note","file":"dir/j.c","line":1,"col":9,"#,
            r#""message":"previous definition is here","group":null,"option":null,"spans":[],"fixits":[]}],"#,
            r#""spans":[{"file":"dir/j.c","line":2,"col":11,"end_line":2,"end_col":18,"label":"new \"body\""},"#,
            r#"{"file":"dir/j.c","line":2,"col":1,"end_line":2,"end_col":8,"label":""}],"#,
            r#""fixits":[{"file":"dir/j.c","line":2,"col":1,"end_line":3,"end_col":1,"text":""},"#,
            r#"{"file":"dir/j.c","line":1,"col":12,"end_line":1,"end_col":12,"text":"\n"}]}"#,
        )
    );
    assert_eq!(
        lines[2],
        r##"{"id":"pp_endif_without_if","severity":"error","file":null,"line":null,"col":null,"message":"#endif without #if","group":null,"option":null,"notes":[],"spans":[],"fixits":[]}"##
    );
}

// spec: diag.md §10.1 (macro notes, fatal severity, -Werror option)
#[test]
fn macro_notes_and_levels() {
    let (sm, f) = source("m.c", "#define BAD bad\nint w = BAD;\n");
    let s = &sm;
    let lines = json(s, &["-Werror", "-Wfatal-errors"], |e| {
        let bad = mac(s, at(s, f, 1, 13), 3, sp(s, f, 2, 9, 3), sp(s, f, 2, 9, 3));
        // A fix-it inside a macro expansion is dropped, like clang's
        // parseable fix-its.
        Diag::new(DiagId::pp_warning_directive, sub(bad, 0, 3))
            .arg("tab\there")
            .fixit(FixIt::replace(sub(bad, 0, 3), "good"))
            .emit(e);
        Diag::new(DiagId::pp_warning_directive, sp(s, f, 2, 1, 3)).arg("suppressed").emit(e);
    });
    assert_eq!(lines.len(), 1, "{lines:#?}");
    assert_eq!(
        lines[0],
        concat!(
            r#"{"id":"pp_warning_directive","severity":"fatal","file":"m.c","line":2,"col":9,"#,
            r##""message":"tab\there","group":"#warnings","option":"-W#warnings","##,
            r#""notes":[{"id":"diag_expanded_from_macro","severity":"note","file":"m.c","line":1,"col":13,"#,
            r#""message":"expanded from macro 'BAD'","group":null,"option":null,"spans":[],"fixits":[]}],"#,
            r#""spans":[],"fixits":[]}"#,
        )
    );
    let lines = json(s, &["-Werror"], |e| {
        Diag::new(DiagId::pp_warning_directive, sp(s, f, 2, 1, 3)).arg("x").emit(e);
    });
    assert!(lines[0].contains(r#""severity":"error""#), "{}", lines[0]);
    assert!(lines[0].contains(r#""option":"-Werror,-W#warnings""#), "{}", lines[0]);
}

// spec: diag.md §10.1 (every line is one object; output is valid JSON)
#[test]
fn lines_are_balanced() {
    let (sm, f) = source("q.c", "\"{[\\\n");
    let s = &sm;
    let lines = json(s, &[], |e| {
        Diag::new(DiagId::pp_warning_directive, sp(s, f, 1, 1, 4)).arg("\"{[\\\u{0}]}").emit(e);
    });
    assert_eq!(lines.len(), 1);
    // Outside strings, braces and brackets balance.
    let (mut depth, mut in_str, mut esc) = (0i32, false, false);
    for c in lines[0].chars() {
        match (in_str, esc, c) {
            (true, true, _) => esc = false,
            (true, false, '\\') => esc = true,
            (true, false, '"') | (false, _, '"') => in_str = !in_str,
            (false, _, '{' | '[') => depth += 1,
            (false, _, '}' | ']') => depth -= 1,
            _ => {}
        }
        assert!(c >= ' ', "raw control character in output");
    }
    assert_eq!((depth, in_str), (0, false));
}
