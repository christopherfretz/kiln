//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 104:44 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 108:15 error "constexpr function never produces a constant expression"
//@ diag: 108:29 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 109:15 error "constexpr function never produces a constant expression"
//@ diag: 109:34 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 110:15 error "constexpr function never produces a constant expression"
//@ diag: 110:38 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 111:16 error "constexpr function never produces a constant expression"
//@ diag: 111:28 note "non-constexpr function 'gv' cannot be used in a constant expression"
//@ diag: 101:6 note "declared here"
//@ diag: 112:15 error "constexpr function never produces a constant expression"
//@ diag: 112:32 note "read of non-const variable 'glob' is not allowed in a constant expression"
//@ diag: 102:5 note "declared here"
//@ diag: 113:15 error "constexpr function never produces a constant expression"
//@ diag: 113:35 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 114:15 error "constexpr function never produces a constant expression"
//@ diag: 114:27 note "subexpression not valid in a constant expression"
//@ diag: 115:15 error "constexpr function never produces a constant expression"
//@ diag: 115:22 note "subexpression not valid in a constant expression"
//@ diag: 116:15 error "constexpr function never produces a constant expression"
//@ diag: 116:34 note "both arms of conditional operator are unable to produce a constant expression"
//@ diag: 117:15 error "constexpr function never produces a constant expression"
//@ diag: 117:46 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 118:15 error "constexpr function never produces a constant expression"
//@ diag: 118:41 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 119:15 error "constexpr function never produces a constant expression"
//@ diag: 119:35 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 120:15 error "constexpr function never produces a constant expression"
//@ diag: 120:45 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 121:15 error "constexpr function never produces a constant expression"
//@ diag: 121:49 note "non-constexpr function 'gv' cannot be used in a constant expression"
//@ diag: 101:6 note "declared here"
//@ diag: 122:15 error "constexpr function never produces a constant expression"
//@ diag: 122:46 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 123:15 error "constexpr function never produces a constant expression"
//@ diag: 123:39 note "non-constexpr function 'gv' cannot be used in a constant expression"
//@ diag: 101:6 note "declared here"
//@ diag: 124:15 error "constexpr function never produces a constant expression"
//@ diag: 124:43 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 125:15 error "consteval function never produces a constant expression"
//@ diag: 125:30 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 126:15 error "consteval function never produces a constant expression"
//@ diag: 126:35 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 129:17 error "constexpr function never produces a constant expression"
//@ diag: 129:31 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 130:17 error "constexpr function never produces a constant expression"
//@ diag: 130:41 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 131:13 error "constexpr constructor never produces a constant expression"
//@ diag: 131:19 note "non-constexpr function 'gv' cannot be used in a constant expression"
//@ diag: 101:6 note "declared here"
//@ diag: 132:13 error "constexpr constructor never produces a constant expression"
//@ diag: 132:26 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 133:24 error "constexpr function never produces a constant expression"
//@ diag: 133:43 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 134:13 error "constexpr function never produces a constant expression"
//@ diag: 134:43 note "read of non-const variable 'glob' is not allowed in a constant expression"
//@ diag: 102:5 note "declared here"
//@ diag: 136:22 error "constexpr function never produces a constant expression"
//@ diag: 136:29 note "non-constexpr function 'gv' cannot be used in a constant expression"
//@ diag: 101:6 note "declared here"
//@ diag: 137:35 error "constexpr constructor never produces a constant expression"
//@ diag: 137:20 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 138:42 error "constexpr constructor never produces a constant expression"
//@ diag: 138:50 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 140:26 error "constexpr constructor never produces a constant expression"
//@ diag: 140:34 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 141:11 error "constexpr function never produces a constant expression"
//@ diag: 141:40 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 100:5 note "declared here"
//@ diag: 104:44 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
// [dcl.constexpr]/6: a constexpr or consteval function no call of which is
// a constant expression is ill-formed, no diagnostic required. clang (and
// kiln) evaluates the body once with the parameters and `this` unknown and
// reports, at the function's name, the first failure in the body itself
// that does not come from an unknown value (clang `isPotentialConstantExpr`;
// an error by default before C++23, -Winvalid-constexpr). WP-873.
int g();
void gv();
int glob;
constexpr int ok() { return 1; }
constexpr int callee_fails() { return ok() + 2147483647; }
constexpr int undef();
// Diagnosed: a call of a non-constexpr function, a read of a non-const
// global, a throw, in any position on the known path.
constexpr int f1() { return g(); }
constexpr int f2(int x) { return g() + x; }
constexpr int f3(int x) { return x + g(); }
constexpr void f4(int x) { gv(); }
constexpr int f5(int) { return glob; }
constexpr int f6(int x) { int a = g(); return a + x; }
constexpr int f7(int x) { throw x; return 0; }
constexpr int f8() { throw 1; return 0; }
constexpr int f9(int x) { return x ? g() : g(); }
constexpr int f10(int x) { int a = x; return g(); }
constexpr int f11(int *p) { return *p + g(); }
constexpr int f12(int x) { return g() > 0 && x; }
constexpr int f13(int x) { if (true) return g(); return x; }
constexpr int f14(int x) { if (false) return x; gv(); return x; }
constexpr int f15(int x) { static_cast<void>(g()); return x; }
constexpr int f16(int x) { for (;;) { gv(); } return x; }
constexpr int f17(int x) { int y = 0; y = g(); return y + x; }
consteval int f18() { return g(); }
consteval int f19(int x) { return g() + x; }
struct S {
  int m;
  constexpr int mf() { return g(); }
  constexpr int mm() const { return m + g(); }
  constexpr S() { gv(); }
  constexpr S(int x) : m(g()) {}
  static constexpr int sf(int x) { return g(); }
  constexpr operator int() const { return glob; }
};
struct D { constexpr ~D() { gv(); } };
struct E { int m = g(); constexpr E() {} };
struct F { constexpr F(int) {} constexpr F() : F(g()) {} };
struct B { constexpr B(int) {} };
struct G : B { constexpr G() : B(g()) {} };
auto l1 = [](int x) constexpr { return g(); };
// Not diagnosed: any call of a constexpr function (its value is unknown, as
// in clang's CheckCallLimit), anything past an unknown condition, an unknown && operand or an unknown argument,
// an uninitialized read, a leak, a lambda not declared constexpr, a
// constructor body after an unknown mem-initializer, a template.
constexpr int n1() { return callee_fails(); }
constexpr int n2() { return undef(); }
constexpr int n3(int x) { if (x) return 1; return g(); }
constexpr int n4(int x) { while (x) gv(); return g(); }
constexpr int n5(int x) { switch (x) { case 1: return 1; } return g(); }
constexpr int n6(int x) { return x > 0 && g(); }
constexpr int n7(int x) { return x + callee_fails(); }
constexpr int n8() { int x; return x; }
constexpr int n9() { int *p = new int(1); return *p; }
constexpr int n10(int x) { auto l = [] { return g(); }; return l() + x; }
constexpr bool n11() { return __builtin_is_constant_evaluated(); }
auto n12 = [](int x) { return g(); };
struct H { int a; constexpr H(int x) : a(x) { gv(); } };
template <class T> constexpr int n13() { return g(); }
template <class T> constexpr int n14(T) { return g(); }
int i = n13<int>() + n14(1);
