// [dcl.fct] (WP-234): -Wunused-but-set-parameter, a
// named parameter referenced only as the target of discarded assignments,
// emitted as the body ends, ahead of the function scope's unused variables.
// A parameter is never a placeholder; an instantiation is not re-checked;
// an error in the function silences it.
//@ args: -Wunused-but-set-parameter -Wunused-but-set-variable
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:13 warning "parameter 'x' set but not used"
//@ diag: 42:13 warning "parameter 'x' set but not used"
//@ diag: 44:13 warning "parameter 'x' set but not used"
//@ diag: 49:13 warning "parameter 'x' set but not used"
//@ diag: 49:22 warning "variable 'y' set but not used"
//@ diag: 51:33 warning "parameter 'x' set but not used"
//@ diag: 53:19 warning "parameter 'x' set but not used"
//@ diag: 54:18 warning "parameter 'x' set but not used"
//@ diag: 54:43 warning "parameter 'x' set but not used"
//@ diag: 55:14 warning "parameter 'x' set but not used"
//@ diag: 57:14 warning "parameter 'x' set but not used"
//@ diag: 58:21 warning "parameter 'y' set but not used"
//@ diag: 59:15 warning "parameter 'p' set but not used"
//@ diag: 60:13 warning "parameter 'e' set but not used"
//@ diag: 61:14 warning "parameter '_' set but not used"
//@ diag: 62:14 warning "parameter 'x' set but not used"
//@ diag: 64:14 warning "parameter 'x' set but not used"
//@ diag: 65:14 warning "parameter 'x' set but not used"
//@ diag: 66:31 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 68:13 warning "parameter 'x' set but not used"
//@ diag: 68:25 warning "non-void function does not return a value"
//@ diag: 69:14 warning "parameter 'x' set but not used"
//@ diag: 69:30 warning "variable 'y' set but not used"
//@ diag: 72:14 warning "parameter 'x' set but not used"
//@ diag: 75:41 warning "parameter 'y' set but not used"
//@ diag: 75:14 warning "parameter 'x' set but not used"
//@ diag: 76:32 warning "variable 'q' set but not used"
//@ diag: 76:14 warning "parameter 'x' set but not used"
//@ diag: 77:26 error "use of undeclared identifier 'undeclared'"
//@ diag: 78:14 warning "parameter 'x' set but not used"
struct C { int v; C& operator=(int); };
enum En { X };
void f1(int x) { x = 1; }
void f2(int x) { x++; }
void f3(int x) { x = 1; (void)x; }
void f4(int x) { x += 1; }
void f5(int &x) { x = 1; }
void f6(C c) { c = 1; }
void f7([[maybe_unused]] int x) { x = 1; }
void f8(int) { }
void f9(int x) { int y; y = 1; x = 2; }
template <class T> void f10(T x) { x = 1; }
template <class T> void f11(int x) { x = 1; }
void f12() { f10(1); f11<int>(1); }
auto f13 = [](int x) { x = 1; };
struct S { S(int x) { x = 1; } void m(int x) { x = 1; } };
void f14(int x = 0) { x = 1; }
void f15(int x) { if ((x = 1)) {} }
void f17(int x) { x = 1; } void f17b(int x);
void f18(int x, int y) { y = x; }
void f19(int *p) { p = 0; }
void f20(En e) { e = X; }
void f21(int _) { _ = 1; }
void f22(int x) { x = 1; x = 2; }
void f23(int x) { int *p = &x; (void)p; }
void f24(int x) { x = 1; return; }
void f25(int x) try { x = 1; } catch (...) {}
void f26(int x) { (void)sizeof(x = 1); }
void f27(int x) = delete;
int f28(int x) { x = 1; }
void f29(int x) { x = 1; int y; y = 2; }
void f30(int x) { switch (x = 1) { case 1: ; } }
void f31(int x) { int y; if ((y = 1)) {} if ((x = 1)) {} }
void f32(int x[2]) { x = 0; }
void f33(int x) { [&] { x = 1; }(); }
void f34(int x) { [=]() mutable { x = 1; }(); }
void f35(int x) { struct L { void m(int y) { y = 1; } }; x = 1; }
void f36(int x) { x = 1; { int q; q = 1; } }
void f37(int x) { x = 1; undeclared(); }
void f38(int x) { x = 1; }
