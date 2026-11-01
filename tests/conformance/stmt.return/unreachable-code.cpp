// [stmt.return]/1, [stmt.goto], [dcl.attr.noreturn], [except.throw]
// (WP-234): -Wunreachable-code-aggressive reports each region of code no
// path from the function's entry reaches (clang `ReachableCode.cpp`), at
// the first element evaluated in it: the callee of a call, the right
// operand of `=` but the left of `+=`, the operator of an overloaded
// operator call, a declaration's initializer. A comma, an implicit
// initializer, a macro expansion and `__builtin_unreachable` are never
// reported; a dead `return` is "'return' will never be executed" unless
// its value branches. Regions come out last first; a later region
// reached from an earlier dead one is part of it. Not in a template
// instantiation.
//@ args: -Wunreachable-code-aggressive
//@ strict-diags
//@ diag: 59:21 warning "code will never be executed"
//@ diag: 60:20 warning "code will never be executed"
//@ diag: 61:22 warning "code will never be executed"
//@ diag: 62:21 warning "code will never be executed"
//@ diag: 63:30 warning "code will never be executed"
//@ diag: 64:26 warning "code will never be executed"
//@ diag: 65:26 warning "code will never be executed"
//@ diag: 66:24 warning "code will never be executed"
//@ diag: 67:30 warning "code will never be executed"
//@ diag: 68:35 warning "code will never be executed"
//@ diag: 69:33 warning "code will never be executed"
//@ diag: 70:45 warning "code will never be executed"
//@ diag: 71:23 warning "code will never be executed"
//@ diag: 72:22 warning "code will never be executed"
//@ diag: 73:30 warning "code will never be executed"
//@ diag: 74:43 warning "code will never be executed"
//@ diag: 75:28 warning "code will never be executed"
//@ diag: 76:27 warning "code will never be executed"
//@ diag: 77:28 warning "code will never be executed"
//@ diag: 78:29 warning "code will never be executed"
//@ diag: 79:22 warning "code will never be executed"
//@ diag: 80:32 warning "code will never be executed"
//@ diag: 83:35 warning "'return' will never be executed"
//@ diag: 84:35 warning "'return' will never be executed"
//@ diag: 85:35 warning "code will never be executed"
//@ diag: 86:47 warning "code will never be executed"
//@ diag: 87:54 warning "code will never be executed"
//@ diag: 88:56 warning "code will never be executed"
//@ diag: 89:25 warning "code will never be executed"
//@ diag: 90:32 warning "code will never be executed"
//@ diag: 91:41 warning "code will never be executed"
//@ diag: 92:46 warning "code will never be executed"
//@ diag: 93:31 warning "code will never be executed"
//@ diag: 94:34 warning "code will never be executed"
//@ diag: 95:40 warning "code will never be executed"
//@ diag: 96:35 warning "code will never be executed"
//@ diag: 97:41 warning "code will never be executed"
//@ diag: 98:33 warning "'return' will never be executed"
//@ diag: 100:17 warning "code will never be executed"
//@ diag: 101:18 warning "code will never be executed"
//@ diag: 102:28 warning "code will never be executed"
int g(int);
[[noreturn]] void die();
#define DEAD g(9)
struct S { int m; void h(); S operator+(S); };
void a1() { return; g(1); }
void a2() { die(); g(1); }
void a3() { throw 1; g(1); }
void a4() { goto L; g(1); L: g(2); }
void a5(int x) { return; x = 2; }
void a6(int x) { return; x += 2; }
void a7(S s) { return; s + s; }
void a8(S s) { return; s.h(); }
void a9(S s) { return; s.m = 1; }
void a10(int *p) { return; p[1] = 2; }
void a11(int x) { return; (void)x; }
void a12(int x) { return; static_cast<void>(x); }
void a13() { return; S{}; }
void a14() { return; int y; (void)y; }
void a15() { return; int y = g(1); (void)y; }
void a16() { return; using I = int; I y = 2; (void)y; }
void a17() { return; try { g(1); } catch (...) {} }
void a18(int x) { return; x ? g(1) : g(2); }
void a19(int x) { return; -x; }
void a20(int x) { return; ++x; }
void a21() { return; [] {}; }
void a22(int x) { g(1), die(), g(2); }
void a23() { if (1) return; DEAD; }
void a24() { return; __builtin_unreachable(); }
int a25(int x) { return x; return g(1); }
int a26(int x) { return x; return g(1) + 1; }
int a27(int x) { return x; return x ? 1 : 2; }
void a28(int x) { if (x) return; else return; g(1); }
void a29(int x) { if (x) { return; } else { die(); } g(1); g(2); }
void a30() { goto L; M: g(1); return; L: g(2); return; g(3); goto M; }
void a31() { return; L: g(1); goto L; }
void a32(int x) { try { die(); g(1); } catch (...) { g(2); } }
void a33(int x) { auto l = [] { return; g(1); }; l(); }
void a34(int x) { if (x) return; else die(); g(1); }
void a35(int x) { return; if (x) g(1); }
void a36(int x) { return; while (x) g(1); }
void a37(int x) { return; for (int i = 0; i < x; ++i) g(i); }
void a38(int x) { return; switch (x) { case 1: g(1); } }
void a39(int x) { while (x) { continue; g(1); } }
void a40() { for (;;) { g(1); } return; }
struct T {
  T() { return; g(1); }
  ~T() { return; g(2); }
  int m(int x) { return x; g(3); }
};
template <class U> int t1(U) { return 0; g(1); }
template <class U> int t2(U) { if (sizeof(U) == 4) return 0; return g(1); }
int i1 = t1(1) + t2(1);
