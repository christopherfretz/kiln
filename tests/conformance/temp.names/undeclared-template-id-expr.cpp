// [temp.names]/3.3: an undeclared unqualified name followed by `<` is a
// template-name, so the list after it holds template arguments. In an
// expression the template-id names nothing: one "use of undeclared
// identifier" and an invalid operand, around which the enclosing
// expression goes on parsing without further diagnostics (no "expected
// '(' for function-style cast"). As a nested-name-specifier it is "no
// template named". Clang 19 and 22 agree on these forms;
// undeclared-template-id-expr-22.cpp holds the ones they recover from
// differently, undeclared-template-id-stmt.cpp the statement forms.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:23 error "use of undeclared identifier 'u1'"
//@ diag: 31:19 error "use of undeclared identifier 'u2'"
//@ diag: 32:18 error "use of undeclared identifier 'u3'"
//@ diag: 33:19 error "use of undeclared identifier 'u4'"
//@ diag: 34:12 error "no template named 'u5'"
//@ diag: 34:31 error "no template named 'u6'"
//@ diag: 35:19 error "use of undeclared identifier 'u7'"
//@ diag: 36:18 error "use of undeclared identifier 'u8'"
//@ diag: 37:18 error "no template named 'u9'"
//@ diag: 38:18 error "use of undeclared identifier 'u10'"
//@ diag: 39:21 error "use of undeclared identifier 'u11'"
//@ diag: 40:18 error "use of undeclared identifier 'u12'"
//@ diag: 40:34 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 41:24 error "use of undeclared identifier 'u13'"
//@ diag: 41:39 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 42:21 error "use of undeclared identifier 'u14'"
//@ diag: 42:36 error "cannot initialize a variable of type 'int' with an lvalue of type 'const char[2]'"
//@ diag: 43:24 error "use of undeclared identifier 'u15'"
int a(int b) { return u1 < b; }
int c() { return (u2<int> * 2); }
int d() { return u3<int>(3); }
int e() { int x = u4<int>; return x; }
void f() { u5<int> * p; const u6<int> &q = 0; }
int g() { return (u7<int>) + 2; }
int h() { return u8<int>[3]; }
int i() { return u9<int>::x; }
int j() { return u10<int, 3> + 1; }
int k() { auto x = &u11<1>; return 0; }
int l() { return u12<int> + ("a" * 2); }
void m(int b) { (void)(u13 < b && "a" * 2); }
void n() { int y = (u14<int>) + 1, z = "s"; }
int o(int b) { return (u15 < b > ) + 1; }
int ok = 0;
