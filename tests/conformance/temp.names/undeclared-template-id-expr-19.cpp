// [temp.names]/3.3: undeclared-template-id-expr-22.cpp's forms under clang
// 19, which models the undeclared template-id as a delayed typo, valid
// until corrected at the end of the full-expression: the tokens after it
// are parsed on, so "expected ')'" / "expected ';' after return statement"
// follow the undeclared name.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:33 error "expected ')'"
//@ diag: 22:23 note "to match this '('"
//@ diag: 22:24 error "use of undeclared identifier 'v1'"
//@ diag: 23:18 error "use of undeclared identifier 'v2'"
//@ diag: 23:25 error "expected ';' after return statement"
//@ diag: 24:27 error "expected ')'"
//@ diag: 24:18 note "to match this '('"
//@ diag: 24:19 error "use of undeclared identifier 'v3'"
//@ diag: 25:33 error "expected ')'"
//@ diag: 25:23 note "to match this '('"
//@ diag: 25:24 error "use of undeclared identifier 'v4'"
//@ diag: 26:18 error "use of undeclared identifier 'v5'"
//@ diag: 26:25 error "expected ';' after return statement"
int a(int b) { return (v1 < b > 2) + 1; }
int c() { return v2<int>{} + 1; }
int d() { return (v3<int> 5); }
int e(int b) { return (v4 < b > 2); }
int f() { return v5<int>{}; }
int ok = 0;
