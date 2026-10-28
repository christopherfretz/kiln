//! Tests for the parser's token source: phase-7 tokens from the
//! preprocessor ([lex.pptoken], [cpp]), pragmas ([cpp.pragma]) and module
//! directives ([cpp.module], [cpp.import]).

mod common;
use common::{parse, parse_with};
use kiln_parse::ParseOptions;

/// [cpp.replace], [lex.key]: macro-expanded tokens are classified as
/// keywords by the parser.
#[test]
fn cpp_replace_tokens_reach_parser() {
    let p = parse("#define T unsigned long\n#define DECL(t, n) t n = 1;\nDECL(T, x)");
    p.assert_clean();
    p.assert_dump("VarDecl x 'unsigned long'");
}

/// [lex.digraph]/2: an alternative token produced by a macro is an
/// operator.
#[test]
fn lex_digraph_alternative_token_from_macro() {
    let p = parse("#define AND and\nconstexpr bool b = true AND false;\nstatic_assert(not b);");
    p.assert_clean();
}

/// [cpp.pragma]: `#pragma pack(...)` is recorded but not applied yet, so it
/// is reported as unsupported.
#[test]
fn cpp_pragma_pack_unsupported() {
    let p = parse("#pragma pack(push, 1)\nstruct S { char c; };\n#pragma pack(pop)\n");
    p.assert_diag("1:9 error: '#pragma pack' is not supported yet");
    p.assert_diag("3:9 error: '#pragma pack' is not supported yet");
}

/// [cpp.pragma]: `#pragma pack` without `(` is ignored with a warning.
#[test]
fn cpp_pragma_pack_missing_lparen() {
    let p = parse("#pragma pack 4\nint x;");
    p.assert_diag("1:14 warning: missing '(' after '#pragma pack' - ignoring");
    assert_eq!(p.problems().len(), 1, "{:?}", p.diags);
}

/// [cpp.pragma]: unbalanced `#pragma GCC visibility pop`.
#[test]
fn cpp_pragma_visibility_pop_unbalanced() {
    let p = parse(
        "#pragma GCC visibility push(hidden)\n#pragma GCC visibility pop\n\
         #pragma GCC visibility pop\nint x;",
    );
    p.assert_diag("3:13 error: #pragma visibility pop with no matching #pragma visibility push");
    assert_eq!(p.problems().len(), 1, "{:?}", p.diags);
}

/// [cpp.pragma]: an unknown visibility is ignored (and not pushed).
#[test]
fn cpp_pragma_visibility_unknown() {
    let p = parse("#pragma GCC visibility push(bogus)\n#pragma GCC visibility pop\nint x;");
    p.assert_diag("1:13 warning: unknown visibility 'bogus'");
    p.assert_diag("2:13 error: #pragma visibility pop");
}

/// [cpp.pragma]: `clang diagnostic` and `GCC diagnostic` are accepted
/// without a diagnostic.
#[test]
fn cpp_pragma_diagnostic_ignored() {
    let p = parse(
        "#pragma clang diagnostic push\n#pragma GCC diagnostic ignored \"-Wall\"\n\
         #pragma clang diagnostic pop\nint x;",
    );
    p.assert_clean();
}

/// [cpp.pragma.op]: `_Pragma` produces the same pragma handling.
#[test]
fn cpp_pragma_op_pack() {
    let p = parse("_Pragma(\"pack(1)\") int x;");
    p.assert_diag("error: '#pragma pack' is not supported yet");
    p.assert_dump("VarDecl x 'int'");
}

/// [cpp.import], [module.import]: an import is reported as unsupported and
/// its tokens through `;` are not parsed.
#[test]
fn module_import_unsupported() {
    let p = parse("import foo.bar;\nint x;");
    p.assert_diag("1:1 error: a module import is not supported yet");
    assert_eq!(p.problems().len(), 1, "{:?}", p.diags);
    p.assert_dump("VarDecl x 'int'");
}

/// [cpp.module], [module.unit]: a module declaration is reported as
/// unsupported.
#[test]
fn module_declaration_unsupported() {
    let p = parse("export module m;\nint x;");
    p.assert_diag("error: a module declaration is not supported yet");
    p.assert_dump("VarDecl x 'int'");
}

/// The bounded lookahead window and the whole-file buffer
/// (`ParseOptions::keep_tokens`) parse identically.
#[test]
fn window_matches_whole_buffer() {
    let mut src = String::from("#define F(n) int f##n(int a) { return a + n; }\n");
    for i in 0..600 {
        src.push_str(&format!("F({i})\nstatic_assert(sizeof(int) == 4, \"{i}\");\n"));
    }
    src.push_str("struct S { S(int); int a; };\n");
    let window = parse(&src);
    let whole = parse_with(&src, &ParseOptions { keep_tokens: true, ..ParseOptions::default() });
    assert_eq!(window.diags, whole.diags);
    assert_eq!(window.dump, whole.dump);
    window.assert_dump("FunctionDecl f599");
}
