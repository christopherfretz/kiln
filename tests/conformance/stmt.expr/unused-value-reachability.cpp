// [stmt.expr] clang DiagIfReachable: the generic unused-value warnings are
// deferred to the end of the function and dropped when unreachable; the
// unused-comparison and nodiscard warnings are not. Outside a function,
// none in a constexpr or a non-inline static data member's initializer.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 21:31 warning "expression result unused"
//@ diag: 22:39 warning "expression result unused"
//@ diag: 27:18 warning "left operand of comma operator has no effect"
//@ diag: 27:21 warning "expression result unused"
//@ diag: 28:29 warning "equality comparison result unused"
//@ diag: 28:29 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 29:28 warning "equality comparison result unused"
//@ diag: 29:28 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 30:44 warning "expression result unused"
//@ diag: 32:60 warning "expression result unused"
//@ diag: 35:22 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 39:11 warning "left operand of comma operator has no effect"
//@ diag: 43:27 warning "left operand of comma operator has no effect"
//@ diag: 46:20 warning "left operand of comma operator has no effect"
template<class T> void t1() { 1; }
template<class T> void t2() { return; 2; }
template<class T> void t3(T x) { return; x; }
void f3() { return; 3; }
void f4(int x) { if (0) x; }
void f5(int x) { while (1) {} x; }
void f6(int x) { x, 6; return; x, 7; }
void f7(int x) { throw 1; x == 1; }
void f8(int x) { return; x == 1; }
void f10(int x) { if consteval { } return; x; }
void f11(int x) { goto L; x; L:; }
void f12(int x) { switch (2) { case 1: x; break; case 2: x + 1; } }
void f13(int x) { true ? (void)0 : (void)(x, 1); }
[[nodiscard]] int nd();
void f14() { return; nd(); }
void inst() { t3(1); }
int g0 = 0;
constexpr int g1 = (1, 2);
int g2 = (g0, 2);
struct S {
  static const int s1;
  static constexpr int s2 = (3, 4);
  static inline int s3 = (g0, 4);
  static const int s4 = (g0, 5);
};
const int S::s1 = (g0, 5);
