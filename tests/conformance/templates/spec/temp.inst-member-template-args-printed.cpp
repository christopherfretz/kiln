// [expr.ref], [temp.names]: the "in call to" note prints a member
// call's explicitly written template arguments (clang's
// `StmtPrinter::VisitMemberExpr`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:15 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 15:43 note "division by zero"
//@ diag: 19:19 note "in call to 's.get<0>().f(0)'"
//@ diag: 20:15 error "constexpr variable 'y' must be initialized by a constant expression"
//@ diag: 15:43 note "division by zero"
//@ diag: 17:52 note "in call to 'S().get<1>().f(0)'"
//@ diag: 20:19 note "in call to 'g<int>(0)'"
struct S {
  template <int N> constexpr const S &get() const { return *this; }
  constexpr int f(int d) const { return 1 / d; }
};
template <class T> constexpr int g(int d) { return S().get<1>().f(d); }
constexpr S s;
constexpr int x = s.get<0>().f(0);
constexpr int y = g<int>(0);
