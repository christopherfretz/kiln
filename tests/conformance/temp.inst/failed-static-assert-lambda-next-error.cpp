// [temp.inst]/3, [dcl.pre]/10-12, [expr.const]/10: a static_assert that
// fails in a lambda body substituted at instantiation leaves the lambda
// valid (clang `BuildStaticAssertDeclaration`, `TransformLambdaExpr`):
// the constant evaluation that calls it, or reads a variable template
// it initializes, still reports its next, independent error (WP-1247;
// the P3068 exception form is failed-static-assert-lambda-keeps-evaluating.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 43:17 error "static assertion failed due to requirement 'sizeof(Widget) == 0': widget: FIRST_REFUSAL"
//@ diag: 42:44 note "while substituting into a lambda expression here"
//@ diag: 48:16 note "in instantiation of variable template specialization 'rules_v' requested here"
//@ diag: 53:15 note "in instantiation of function template specialization 'bind_site<Widget>' requested here"
//@ diag: 43:27 note "expression evaluates to '1 == 0'"
//@ diag: 53:15 error "static assertion expression is not an integral constant expression"
//@ diag: 50:10 note "non-constexpr function 'second_refusal' cannot be used in a constant expression"
//@ diag: 53:15 note "in call to 'bind_site<Widget>()'"
//@ diag: 33:6 note "declared here"
//@ diag: 57:30 error "static assertion failed due to requirement 'sizeof(int) == 0': gizmo: FIRST_REFUSAL"
//@ diag: 57:14 note "while substituting into a lambda expression here"
//@ diag: 60:15 note "in instantiation of function template specialization 'called<int>' requested here"
//@ diag: 57:40 note "expression evaluates to '4 == 0'"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:20 note "non-constexpr function 'second_refusal' cannot be used in a constant expression"
//@ diag: 60:15 note "in call to 'called<int>()'"
//@ diag: 33:6 note "declared here"
//@ diag: 63:34 error "no member named 'missing' in 'Widget'"
//@ diag: 66:9 note "in instantiation of variable template specialization 'rule_v<Widget>' requested here"
//@ diag: 69:15 note "in instantiation of function template specialization 'plain<Widget>' requested here"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 67:10 note "non-constexpr function 'second_refusal' cannot be used in a constant expression"
//@ diag: 69:15 note "in call to 'plain<Widget>()'"
//@ diag: 33:6 note "declared here"
bool second_refusal(); // not constexpr: a call ends the evaluation

struct span {
  const int *b = nullptr, *e = nullptr;
  constexpr const int *begin() const { return b; }
  constexpr const int *end() const { return e; }
};

template <class T>
inline constexpr auto rules_v = [] -> span {
  static_assert(sizeof(T) == 0, "widget: FIRST_REFUSAL");
  return {};
}();
template <class T>
consteval bool bind_site() {
  for (int r : rules_v<T>)
    (void)r;
  return second_refusal();
}
struct Widget {};
static_assert(bind_site<Widget>());

template <class T>
consteval bool called() {
  int x = [] { static_assert(sizeof(T) == 0, "gizmo: FIRST_REFUSAL"); return 1; }();
  return x == 1 && second_refusal();
}
static_assert(called<int>());

template <class T>
inline constexpr int rule_v = T::missing;
template <class T>
consteval bool plain() {
  (void)rule_v<T>;
  return second_refusal();
}
static_assert(plain<Widget>());
