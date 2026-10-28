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
