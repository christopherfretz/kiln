// [dcl.ambig.res]/1, [dcl.fct]/3: in `void a(u1);` with `u1` undeclared the
// parenthesized clause is neither a valid expression nor a valid parameter
// list; clang resolves it as a function declarator, reports "unknown type
// name 'u1'" and still declares `a`, so later calls check against its
// parameter count instead of cascading. Any token after the name but `(`,
// `::` and `<` takes this path (`u4 *p`, `u5 &r`, `u7 = 1`). Once a parameter
// is invalid the "missing default argument" check is skipped, as in clang.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:8 error "unknown type name 'u1'"
//@ diag: 33:18 error "no matching function for call to 'a'"
//@ diag: 32:6 note "candidate function not viable: requires 1 argument, but 0 were provided"
//@ diag: 34:8 error "unknown type name 'u2'"
//@ diag: 35:13 error "unknown type name 'u3'"
//@ diag: 36:7 error "unknown type name 'u4'"
//@ diag: 38:8 error "unknown type name 'u5'"
//@ diag: 38:15 error "unknown type name 'u6'"
//@ diag: 38:24 error "unknown type name 'u7'"
//@ diag: 38:32 error "unknown type name 'u8'"
//@ diag: 39:12 error "no matching function for call to 'i'"
//@ diag: 38:6 note "candidate function not viable: requires 5 arguments, but 0 were provided"
//@ diag: 40:20 error "unknown type name 'u9'"
//@ diag: 40:38 error "unknown type name 'u10'"
//@ diag: 42:18 error "unknown type name 'u11'"
//@ diag: 43:21 error "missing default argument on parameter"
//@ diag: 46:11 error "unknown type name 'u12'"
//@ diag: 48:9 error "unknown type name 'u14'"
//@ diag: 48:8 warning "parentheses were disambiguated as a function declaration"
//@ diag: 48:9 note "add a pair of parentheses to declare a variable"
//@ diag: 51:7 error "unknown type name 'u15'"
//@ diag: 51:15 error "unknown type name 'u16'"
void a(u1);
void b() { a(1); a(); }
void c(u2, int);
void d(int, u3 x);
int e(u4 *p);
void h() { c(1, 2); d(1, 2); e(0); }
void i(u5 &r, u6 &&rr, u7 = 1, u8 arr[2], int);
void j() { i(); }
struct M { void mf(u9); void mg(int, u10 *); };
void k(M m) { m.mf(1); m.mg(1, 0); m.mf(); }
void i3(int = 1, u11, int);
void i5(int = 1, int);
int n;
void g() {
  void la(u12);
  la(1);
  int x(u14);
  x(1);
}
int q(u15), r(u16 u);
void l() { q(1); r(1); }
