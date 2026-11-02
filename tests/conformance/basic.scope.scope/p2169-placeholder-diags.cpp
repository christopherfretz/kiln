// [basic.scope.scope]/5 (C++26, P2169R4): a name redeclared as `_` in its
// scope is ambiguous on use (clang's wording, the latest declaration
// first, or in member order for a member access); a declaration that is
// not name-independent (a static local, a namespace-scope variable)
// still conflicts.
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 32:43 error "ambiguous reference to placeholder '_', which is defined multiple times"
//@ diag: 32:29 note "placeholder declared here"
//@ diag: 32:18 note "placeholder declared here"
//@ diag: 33:45 error "ambiguous reference to placeholder '_', which is defined multiple times"
//@ diag: 33:24 note "placeholder declared here"
//@ diag: 33:21 note "placeholder declared here"
//@ diag: 34:45 error "ambiguous reference to placeholder '_', which is defined multiple times"
//@ diag: 34:29 note "placeholder declared here"
//@ diag: 34:22 note "placeholder declared here"
//@ diag: 35:57 error "ambiguous reference to placeholder '_', which is defined multiple times"
//@ diag: 35:43 note "placeholder declared here"
//@ diag: 35:34 note "placeholder declared here"
//@ diag: 36:43 error "ambiguous reference to placeholder '_', which is defined multiple times"
//@ diag: 36:23 note "placeholder declared here"
//@ diag: 36:16 note "placeholder declared here"
//@ diag: 37:25 error "ambiguous reference to placeholder '_', which is defined multiple times"
//@ diag: 36:16 note "placeholder declared here"
//@ diag: 36:23 note "placeholder declared here"
//@ diag: 38:43 error "redefinition of '_'"
//@ diag: 38:25 note "previous definition is here"
//@ diag: 40:5 error "redefinition of '_'"
//@ diag: 39:5 note "previous definition is here"
struct P { int a, b; };
int vars() { int _ = 1; int _ = 2; return _; }
int binds() { auto [_, _] = P{1, 2}; return _; }
void caps() { (void)[_ = 1, _ = 2] { return _; }; }
template <class T> T tm(T t) { T _ = t; T _ = t; return _; }
struct S { int _; int _; int f() { return _; } };
int mem(S s) { return s._; }
int stat() { static int _ = 1; static int _ = 2; return 0; }
int _ = 1;
int _ = 2;
