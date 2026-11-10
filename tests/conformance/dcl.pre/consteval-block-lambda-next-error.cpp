// [dcl.pre]/P3289R1, [temp.inst]/3, [expr.const]/10: a consteval block
// that fails in a lambda body substituted at instantiation is one error;
// the lambda stays valid, so the constant evaluation that calls it, or
// reads a variable template it initializes, goes on to report its next,
// independent error (WP-1257, the consteval-block form of
// temp.inst/failed-static-assert-lambda-next-error.cpp; spec/sema-templates.md §5.5).
// GCC 16.1 rejects too, but treats the enclosing bodies as erroneous: its
// second errors carry no note naming second_refusal, and bind_site gets a
// third, "'bind_site<Widget>' called in a constant expression". kiln
// follows the standard: the failed block makes the program ill-formed
// without invalidating the lambda, so only the independent error follows.
//@ expect: compile-fail
//@ skip(clang): clang 19 has no consteval blocks (P3289R1)
//@ oracle: gcc
//@ strict-diags
//@ diag: 43:3 error "consteval block is not a constant expression"
//@ diag: 42:44 note "while substituting into a lambda expression here"
//@ diag: 48:16 note "in instantiation of variable template specialization 'rules_v' requested here"
//@ diag: 53:15 note "in instantiation of function template specialization 'bind_site<Widget>' requested here"
//@ diag: 43:21 note "unhandled exception of type 'int' with content 1 thrown from here"
//@ diag: 53:15 error "static assertion expression is not an integral constant expression"
//@ diag: 50:10 note "non-constexpr function 'second_refusal' cannot be used in a constant expression"
//@ diag: 53:15 note "in call to 'bind_site<Widget>()'"
//@ diag: 33:6 note "declared here"
//@ diag: 57:16 error "consteval block is not a constant expression"
//@ diag: 57:14 note "while substituting into a lambda expression here"
//@ diag: 60:15 note "in instantiation of function template specialization 'called<int>' requested here"
//@ diag: 57:54 note "unhandled exception of type 'int' with content 2 thrown from here"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:20 note "non-constexpr function 'second_refusal' cannot be used in a constant expression"
//@ diag: 60:15 note "in call to 'called<int>()'"
//@ diag: 33:6 note "declared here"
bool second_refusal(); // not constexpr: a call ends the evaluation

struct span {
  const int *b = nullptr, *e = nullptr;
  constexpr const int *begin() const { return b; }
  constexpr const int *end() const { return e; }
};

template <class T>
inline constexpr auto rules_v = [] -> span {
  consteval { throw 1; }
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
  int x = [] { consteval { if (sizeof(T) == 4) throw 2; } return 1; }();
  return x == 1 && second_refusal();
}
static_assert(called<int>());
