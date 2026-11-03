// [dcl.attr.grammar]/5 (implementation attributes) and [expr.const]: an enable_if or
// diagnose_if condition that can never be a constant expression, whatever the
// parameters, is an error at the attribute, which is dropped (clang
// checkFunctionConditionAttr). Evaluation goes on past a failure to read a
// parameter, except past an unknown && / || left operand or ?: condition (a
// ?: with an unknown condition fails only if both arms do). A dependent
// condition is checked by each instantiation, as a hard error even while
// deducing. Verified against clang 19 and clang 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 62:31 error "'enable_if' attribute expression never produces a constant expression"
//@ diag: 62:41 note "non-constexpr function 'rt' cannot be used in a constant expression"
//@ diag: 52:5 note "declared here"
//@ diag: 63:31 error "'enable_if' attribute expression never produces a constant expression"
//@ diag: 63:46 note "non-constexpr function 'rt' cannot be used in a constant expression"
//@ diag: 52:5 note "declared here"
//@ diag: 64:31 error "'enable_if' attribute expression never produces a constant expression"
//@ diag: 64:41 note "read of non-const variable 'g' is not allowed in a constant expression"
//@ diag: 53:5 note "declared here"
//@ diag: 65:42 warning "left operand of comma operator has no effect"
//@ diag: 65:31 error "'enable_if' attribute expression never produces a constant expression"
//@ diag: 65:45 note "non-constexpr function 'rt' cannot be used in a constant expression"
//@ diag: 52:5 note "declared here"
//@ diag: 66:31 error "'enable_if' attribute expression never produces a constant expression"
//@ diag: 66:50 note "non-constexpr function 'rt' cannot be used in a constant expression"
//@ diag: 52:5 note "declared here"
//@ diag: 67:31 error "'enable_if' attribute expression never produces a constant expression"
//@ diag: 67:50 note "read of non-const variable 'g' is not allowed in a constant expression"
//@ diag: 53:5 note "declared here"
//@ diag: 68:31 error "'enable_if' attribute expression never produces a constant expression"
//@ diag: 68:44 note "non-constexpr function 'rt' cannot be used in a constant expression"
//@ diag: 52:5 note "declared here"
//@ diag: 69:31 error "'enable_if' attribute expression never produces a constant expression"
//@ diag: 69:41 note "non-constexpr function 'rt' cannot be used in a constant expression"
//@ diag: 52:5 note "declared here"
//@ diag: 70:31 error "'enable_if' attribute expression never produces a constant expression"
//@ diag: 70:41 note "both arms of conditional operator are unable to produce a constant expression"
//@ diag: 71:32 error "'diagnose_if' attribute expression never produces a constant expression"
//@ diag: 71:49 note "non-constexpr function 'rt' cannot be used in a constant expression"
//@ diag: 52:5 note "declared here"
//@ diag: 72:48 error "'enable_if' attribute expression never produces a constant expression"
//@ diag: 72:58 note "non-constexpr function 'rt' cannot be used in a constant expression"
//@ diag: 52:5 note "declared here"
//@ diag: 86:46 error "'enable_if' attribute expression never produces a constant expression"
//@ diag: 93:22 note "in instantiation of function template specialization 'c1<X>' requested here"
//@ diag: 86:64 note "non-constexpr function 'rt' cannot be used in a constant expression"
//@ diag: 52:5 note "declared here"
//@ diag: 88:47 error "'diagnose_if' attribute expression never produces a constant expression"
//@ diag: 95:3 note "in instantiation of function template specialization 'c2<X>' requested here"
//@ diag: 88:67 note "non-constexpr function 'rt' cannot be used in a constant expression"
//@ diag: 52:5 note "declared here"
int rt();
int g;
const int cg = 3;
constexpr int id(int x) { return x; }
constexpr int cf(int x) { return x > 0 ? x : rt(); }
constexpr int cf2(int) { return rt(); }
constexpr int ar[2] = {1, 2};
struct X { static const int x = 1; };

// Never constant: an operand that names no parameter fails.
void a1(int n) __attribute__((enable_if(rt(), "")));
void a2(int n) __attribute__((enable_if(n == rt(), "")));
void a3(int n) __attribute__((enable_if(g == n, "")));
void a4(int n) __attribute__((enable_if((n, rt()), "")));
void a5(int n) __attribute__((enable_if(ar[n] == rt(), "")));
void a6(int n) __attribute__((enable_if(id(n) == g, "")));
void a7(int n) __attribute__((enable_if(id(rt()) == n, "")));
void a8(int n) __attribute__((enable_if(rt() || n, "")));
void a9(int n) __attribute__((enable_if(n ? rt() : rt() + 1, "")));
void a10(int n) __attribute__((diagnose_if(n == rt(), "d", "warning")));
template<class T> void a11(T n) __attribute__((enable_if(rt(), "")));

// Maybe constant: evaluation stops at the unknown parameter first.
void b1(int n) __attribute__((enable_if(n && rt(), "")));
void b2(int n) __attribute__((enable_if(n ? rt() : 1, "")));
void b3(int n) __attribute__((enable_if(cf(n), "")));
void b4(int n) __attribute__((enable_if(cf2(n), "")));
void b5(int n) __attribute__((enable_if(sizeof(rt()) == 4, "")));
void b6(int n) __attribute__((enable_if(cg == n, "")));
void b7(int n) __attribute__((enable_if(n || g, "")));
void b8(int& n) __attribute__((enable_if(n == 1, "")));
void b9(int n) __attribute__((diagnose_if(n || rt(), "d", "warning")));

// A dependent condition is checked by the instantiation, as a hard error.
template<class T> int c1(T n) __attribute__((enable_if(T::x == rt(), "")));
void c1(...);
template<class T> void c2(T n) __attribute__((diagnose_if(T::x == rt(), "d", "warning")));

void use(int i) {
  a1(1);
  a10(1);
  using R = decltype(c1(X{}));
  R r = 0;
  c2(X{});
  b9(i);
}
