//! Per-rule tests for statements ([stmt]).

mod common;
use common::parse;

/// [stmt.if]: condition converted to bool; condition declarations.
#[test]
fn stmt_if_condition_decl() {
    let p = parse("int g(); int f() { if (int x = g()) return x; else return -x; }");
    p.assert_clean();
    p.assert_dump("IfStmt");
    p.assert_dump("VarDecl used x 'int' cinit");
}

/// [stmt.if]: init-statement.
#[test]
fn stmt_if_init() {
    let p = parse("int f(int a) { if (int b = a * 2; b > 3) return b; return 0; }");
    p.assert_clean();
    p.assert_dump("IfStmt has_init");
}

/// [stmt.pre]: a condition declaration needs an initializer.
#[test]
fn stmt_condition_needs_init() {
    let p = parse("void f() { while (int x) {} }");
    p.assert_diag("variable declaration in condition must have an initializer");
}

/// [stmt.while], [stmt.do], [stmt.for].
#[test]
fn stmt_loops() {
    let p = parse(
        "int f(int n) { int s = 0; while (n) --n; do s++; while (s < 3); \
         for (int i = 0; i < 4; ++i) { if (i == 2) continue; s += i; } for (;;) break; return s; }",
    );
    p.assert_clean();
    p.assert_dump("WhileStmt");
    p.assert_dump("DoStmt");
    p.assert_dump("ForStmt");
    p.assert_dump("ContinueStmt");
    p.assert_dump("BreakStmt");
}

/// [stmt.break], [stmt.cont]: must be inside a loop/switch.
#[test]
fn stmt_break_outside() {
    let p = parse("void f() { break; }");
    p.assert_diag("'break' statement not in loop or switch statement");
    let p = parse("void f() { switch (1) { case 1: continue; } }");
    p.assert_diag("'continue' statement not in loop statement");
}

/// [stmt.switch]: promotion of the condition, duplicate cases.
#[test]
fn stmt_switch() {
    let p = parse("int f(char c) { switch (c) { case 'a': return 1; default: return 0; } }");
    p.assert_clean();
    p.assert_dump("ImplicitCastExpr 'int' <IntegralCast>");
    let p = parse("void f(int x) { switch (x) { case 1: case 1: break; } }");
    p.assert_diag("duplicate case value '1'");
    let p = parse("void f(int x) { switch (x) { default: default: break; } }");
    p.assert_diag("multiple default labels in one switch");
}

/// [stmt.switch]: a case label must be a converted constant expression.
#[test]
fn stmt_switch_case_not_constant() {
    let p = parse("void f(int x, int y) { switch (x) { case y: break; } }");
    p.assert_diag("case value is not a constant expression");
}

/// [stmt.label]: `case` outside a switch.
#[test]
fn stmt_case_outside_switch() {
    let p = parse("void f() { case 1: ; }");
    p.assert_diag("'case' statement not in switch statement");
}

/// [stmt.goto], [stmt.label]: labels have function scope.
#[test]
fn stmt_goto_labels() {
    let p = parse("int f(int n) { goto out; n = 1; out: return n; }");
    p.assert_clean();
    p.assert_dump("GotoStmt");
    p.assert_dump("LabelStmt");
    let p = parse("void f() { goto nowhere; }");
    p.assert_diag("use of undeclared label 'nowhere'");
    let p = parse("void f() { a: ; a: ; }");
    p.assert_diag("redefinition of label 'a'");
}

/// [stmt.return]: value/void mismatches.
#[test]
fn stmt_return() {
    let p = parse("void f() { return 1; }");
    p.assert_diag("void function 'f' should not return a value");
    let p = parse("int f() { return; }");
    p.assert_diag("non-void function 'f' should return a value");
}

/// [stmt.return]: flowing off the end of a value-returning function.
#[test]
fn stmt_return_fall_off() {
    let p = parse("int f(int x) { if (x) return 1; }");
    p.assert_diag("warning: non-void function does not return a value in all control paths");
    // main is exempt ([basic.start.main]).
    let p = parse("int main() { }");
    p.assert_clean();
}

/// [stmt.return]: return value is copy-initialized.
#[test]
fn stmt_return_conversion() {
    let p = parse("double f() { return 1; }");
    p.assert_clean();
    p.assert_dump("ImplicitCastExpr 'double' <IntegralToFloating>");
    let p = parse("int *f() { return 1.0; }");
    p.assert_diag(
        "cannot initialize return object of type 'int *' with an rvalue of type 'double'",
    );
}

/// [stmt.expr]: unused-result warnings.
#[test]
fn stmt_expr_unused() {
    let p = parse("void f(int a) { a + 1; }");
    p.assert_diag("warning: expression result unused");
}

/// [stmt.block]: block-scope names shadow and end with the block.
#[test]
fn stmt_block_scope() {
    let p = parse("int f() { { int y = 1; } return y; }");
    p.assert_diag("use of undeclared identifier 'y'");
}

/// [stmt.dcl]: static locals and local externs.
#[test]
fn stmt_dcl_static_local() {
    let p = parse("int f() { static int n = 0; extern int g; return ++n + g; }");
    p.assert_clean();
    p.assert_dump("VarDecl used n 'int' static cinit");
    p.assert_dump("VarDecl used g 'int' extern");
}

/// Recovery: a missing ';' is reported with clang's wording and parsing resumes.
#[test]
fn stmt_recovery_missing_semi() {
    let p = parse("int f() { int a = 1 return a; }");
    p.assert_diag("expected ';' at end of declaration");
}

/// [stmt.if]/2: `if constexpr` folds its condition (with an init-statement).
#[test]
fn stmt_if_constexpr() {
    let p = parse(
        "int f(int v) { if constexpr (int k = 2; sizeof(int) == 4) { return v + k; } \
         else { return v; } }",
    );
    p.assert_clean();
    p.assert_dump("IfStmt has_init has_else constexpr");
    p.assert_dump("ConstantExpr 'bool' value=true");
    let p = parse("int n; void f() { if constexpr (n) {} }");
    p.assert_diag("constexpr if condition is not a constant expression");
    let p = parse("void f() { if constexpr 1; }");
    p.assert_diag("expected '(' after 'if'");
}

/// [stmt.if]/4: `if consteval` / `if !consteval` with compound branches.
#[test]
fn stmt_if_consteval() {
    let p = parse(
        "constexpr int f(int v) { if consteval { return 1; } else { return v; } }\n\
         int g() { if !consteval { return 2; } return 3; }",
    );
    p.assert_clean();
    p.assert_dump("IfStmt has_else consteval");
    p.assert_dump("IfStmt !consteval");
    let p = parse("void f() { if consteval return; }");
    p.assert_diag("expected { after consteval");
    let p = parse("void f() { if consteval {} else return; }");
    p.assert_diag("expected { after else");
}

/// [stmt.expr]/1: a qualified name lookup does not find is reported at the
/// end of its full-expression (clang's delayed typo correction), so a
/// missing `;` after the expression statement comes first; a declaration's
/// initializer reports it where it is written (WP-308).
#[test]
fn delayed_no_member_after_missing_semi() {
    let p = parse(
        "struct S {};\nnamespace N {}\nvoid g() {\n  S::nom()\n  N::nom()\n  int a = S::none\n}\n",
    );
    let errs: Vec<&str> =
        p.diags.iter().map(String::as_str).filter(|d| d.contains("error")).collect();
    assert_eq!(
        errs,
        [
            "4:11 error: expected ';' after expression",
            "4:6 error: no member named 'nom' in 'S'",
            "5:11 error: expected ';' after expression",
            "5:6 error: no member named 'nom' in namespace 'N'",
            "6:14 error: no member named 'none' in 'S'",
            "6:18 error: expected ';' at end of declaration",
        ],
        "{:?}",
        p.diags
    );
}

/// WP-318: a delayed typo in a full-expression nested inside a larger
/// expression statement. Inside call arguments clang skips to the `)`
/// without "expected ')'"; in parentheses "expected ')'" comes first and
/// `(S::nom x)` is not a cast.
#[test]
fn delayed_no_member_nested_in_expression_statement() {
    let p = parse(
        "struct S {};\nvoid f(int);\nvoid g(int a) {\n  f(S::nom x);\n  (S::nom x);\n  a = (S::nom x);\n}\n",
    );
    let errs: Vec<&str> =
        p.diags.iter().map(String::as_str).filter(|d| d.contains("error")).collect();
    assert_eq!(
        errs,
        [
            "4:8 error: no member named 'nom' in 'S'",
            "5:11 error: expected ')'",
            "5:7 error: no member named 'nom' in 'S'",
            "6:15 error: expected ')'",
            "6:11 error: no member named 'nom' in 'S'",
        ],
        "{:?}",
        p.diags
    );
}

/// WP-330: where clang's delayed typo correction reports `S::nom`, in
/// order: a subscript missing its `]` drops its index's typo until the end
/// of the translation unit (after the pending instantiations); a
/// subscript's base is corrected at the `]`; a call missing its `)`
/// corrects its callee and arguments at once and makes the expression
/// invalid, so an expression statement missing its `;` after it skips to
/// the next `;` with no "expected ';'"; a lambda body's and a
/// statement expression's statements end their own full-expressions
/// (`tests/conformance/stmt.expr/delayed-typo-correction-points.cpp`).
#[test]
fn delayed_typo_correction_points() {
    let p = parse(concat!(
        "struct S { static int m; };\nint arr[3];\nint g(int);\n",
        "template <class T> void tf(T t) { t.zz; }\n",
        "void h(int a) {\n",
        "  a = arr[S::nom1 x];\n",
        "  int v = arr[S::nom2 x];\n",
        "  a = S::nom3[1 2] + ;\n",
        "  S::nom4(1;\n",
        "  a = g(S::nom5 x) + ;\n",
        "  [] { int v = S::nom6; return; }(), 1 +;\n",
        "  ({ int w = S::nom7; w; }) + 5 +;\n",
        "  tf(1);\n",
        "  a = g(S::nom8 x)\n",
        "  a = 1;\n",
        "  ({ g(S::nom9 z) 1; });\n",
        "  a = 2\n",
        "}\n",
    ));
    let errs: Vec<&str> =
        p.diags.iter().map(String::as_str).filter(|d| d.contains("error")).collect();
    assert_eq!(
        errs,
        [
            "6:19 error: expected ']'",
            "7:23 error: expected ']'",
            "8:10 error: no member named 'nom3' in 'S'",
            "8:17 error: expected ']'",
            "8:22 error: expected expression",
            "9:6 error: no member named 'nom4' in 'S'",
            "10:12 error: no member named 'nom5' in 'S'",
            "10:22 error: expected expression",
            "11:19 error: no member named 'nom6' in 'S'",
            "11:41 error: expected expression",
            "12:17 error: no member named 'nom7' in 'S'",
            "12:34 error: expected expression",
            "14:12 error: no member named 'nom8' in 'S'",
            "16:11 error: no member named 'nom9' in 'S'",
            "17:8 error: expected ';' after expression",
            "4:36 error: member reference base type 'int' is not a structure or union",
            "6:14 error: no member named 'nom1' in 'S'",
            "7:18 error: no member named 'nom2' in 'S'",
        ],
        "{:?}",
        p.diags
    );
}
