// [dcl.constexpr]/6 (C++20): clang's never-constant walk
// (`isPotentialConstantExpr`) evaluates a discarded expression as an lvalue
// when it is a glvalue, so naming a by-value parameter, its member or its
// address, a comma or a void cast of one notes no side effect and the walk
// reaches the static variable; a reference parameter, `this`, or reading a
// parameter's value notes one, and the walk then fails quietly at the end
// of `y`'s lifetime (WP-1150).
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -Wno-gnu-statement-expression -Wno-uninitialized -ferror-limit=0
//@ strict-diags
//@ diag: 29:15 error "constexpr function never produces a constant expression"
//@ diag: 29:59 note "control flows through the definition of a static variable"
//@ diag: 30:15 error "constexpr function never produces a constant expression"
//@ diag: 30:67 note "control flows through the definition of a static variable"
//@ diag: 31:15 error "constexpr function never produces a constant expression"
//@ diag: 31:64 note "control flows through the definition of a static variable"
//@ diag: 32:15 error "constexpr function never produces a constant expression"
//@ diag: 32:75 note "control flows through the definition of a static variable"
//@ diag: 33:15 error "constexpr function never produces a constant expression"
//@ diag: 33:60 note "control flows through the definition of a static variable"
//@ diag: 34:15 error "constexpr function never produces a constant expression"
//@ diag: 34:62 note "control flows through the definition of a static variable"

struct S { int v; };

// Diagnosed: a discarded parameter, its member or its address is an
// lvalue evaluation that notes no side effect, so destroying `y` succeeds.
constexpr int named(int x) { x; { int y = 0; } static int i = 0; return i; }
constexpr int to_void(int x) { (void)x; { int y = 0; } static int i = 0; return i; }
constexpr int comma(int x) { (x, x); { int y = 0; } static int i = 0; return i; }
constexpr int comma_temporary(int x) { x, S{1}; { int y = 0; } static int i = 0; return i; }
constexpr int member(S s) { s.v; { int y = 0; } static int i = 0; return i; }
constexpr int address(int x) { &x; { int y = 0; } static int i = 0; return i; }

// Not diagnosed: a reference parameter, a read and a conditional over
// parameters, and `this` note a side effect, so destroying `y` fails.
constexpr int reference(const int &x) { (void)x; { int y = 0; } static int i = 0; return i; }
constexpr int read(int x) { x + 1; { int y = 0; } static int i = 0; return i; }
constexpr int conditional(int x) { x ? x : x; { int y = 0; } static int i = 0; return i; }
struct T {
  int v;
  constexpr int implicit_this(int x) { v; { int y = 0; } static int i = 0; return i; }
};
