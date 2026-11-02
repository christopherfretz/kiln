// [temp.names]/3.3: undeclared-template-id-expr.cpp's rule where clang 19
// adds "expected ')'" / "expected ';'" after the first error (its
// delayed typo correction); clang 22 reports the undeclared name alone.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:24 error "use of undeclared identifier 'v1'"
//@ diag: 13:18 error "use of undeclared identifier 'v2'"
//@ diag: 14:19 error "use of undeclared identifier 'v3'"
//@ diag: 15:24 error "use of undeclared identifier 'v4'"
//@ diag: 16:18 error "use of undeclared identifier 'v5'"
int a(int b) { return (v1 < b > 2) + 1; }
int c() { return v2<int>{} + 1; }
int d() { return (v3<int> 5); }
int e(int b) { return (v4 < b > 2); }
int f() { return v5<int>{}; }
int ok = 0;
