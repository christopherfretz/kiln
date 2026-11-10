// [temp.inst]/3, [dcl.pre]/10-12, [expr.const]/10 (C++26, P3068R6): a
// static_assert that fails in a lambda body substituted at instantiation
// is a failed declaration, not an invalid body (clang's
// `BuildStaticAssertDeclaration` keeps it; `TransformLambdaExpr` fails
// only for an invalid body). The lambda-expression stays valid, so a
// variable template initialized by calling it still has a value, and a
// constant evaluation that uses it goes on to report the next,
// independent error: here an uncaught exception, with its what() text.
// GCC 16.1 reports the same errors (WP-1247; kiln failed the lambda and
// the second refusal was a bare "not an integral constant expression").
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-fail
//@ strict-diags
//@ diag: 58:17 error "static assertion failed due to requirement 'sizeof(Widget) == 0': widget: FIRST_REFUSAL"
//@ diag: 57:44 note "while substituting into a lambda expression here"
//@ diag: 63:16 note "in instantiation of variable template specialization 'rules_v' requested here"
//@ diag: 68:15 note "in instantiation of function template specialization 'bind_site<Widget>' requested here"
//@ diag: 58:27 note "expression evaluates to '1 == 0'"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:54 note "unhandled exception: widget: SECOND_REFUSAL"
//@ diag: 73:31 error "static assertion failed due to requirement 'sizeof(int) == 0': gadget: FIRST_REFUSAL"
//@ diag: 73:15 note "while substituting into a lambda expression here"
//@ diag: 77:15 note "in instantiation of function template specialization 'made_only<int>' requested here"
//@ diag: 73:41 note "expression evaluates to '4 == 0'"
//@ diag: 77:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:54 note "unhandled exception: widget: SECOND_REFUSAL"
//@ diag: 82:30 error "static assertion failed due to requirement 'sizeof(int) == 0': gizmo: FIRST_REFUSAL"
//@ diag: 82:14 note "while substituting into a lambda expression here"
//@ diag: 85:15 note "in instantiation of function template specialization 'called<int>' requested here"
//@ diag: 82:40 note "expression evaluates to '4 == 0'"
//@ diag: 85:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:54 note "unhandled exception: widget: SECOND_REFUSAL"
namespace std {
struct exception {
  constexpr exception() noexcept {}
  constexpr virtual ~exception() {}
  constexpr virtual const char *what() const noexcept { return "std::exception"; }
};
} // namespace std
struct refusal : std::exception {
  const char *msg;
  constexpr refusal(const char *m) : msg(m) {}
  constexpr const char *what() const noexcept override { return msg; }
};
[[noreturn]] consteval void raise(const char *msg) { throw refusal(msg); }
consteval bool second_refusal() { raise("widget: SECOND_REFUSAL"); }

struct span {
  const int *b = nullptr, *e = nullptr;
  constexpr const int *begin() const { return b; }
  constexpr const int *end() const { return e; }
};

// The reported shape: a variable template initialized by an immediately
// invoked lambda whose body fails a static_assert.
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

// The lambda in a function template's body: only made, not called.
template <class T>
consteval bool made_only() {
  auto l = [] { static_assert(sizeof(T) == 0, "gadget: FIRST_REFUSAL"); return 1; };
  (void)l;
  return second_refusal();
}
static_assert(made_only<int>());

// The lambda called in a function template's body; its value is used.
template <class T>
consteval bool called() {
  int x = [] { static_assert(sizeof(T) == 0, "gizmo: FIRST_REFUSAL"); return 1; }();
  return x == 1 && second_refusal();
}
static_assert(called<int>());
