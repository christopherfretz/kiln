// [temp.inst]/7, [dcl.pre]/10-12, [expr.const]/10 (C++26, P3068R6): a
// variable template specialization whose initializer fails at
// instantiation (a missing member; a call of a function whose
// static_assert fails) does not stop the constant evaluation that
// named it: the next, independent error is still reported, an uncaught
// exception with its what() text, as GCC 16.1 does (WP-1247).
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:34 error "no member named 'missing' in 'Widget'"
//@ diag: 42:9 note "in instantiation of variable template specialization 'rule_v<Widget>' requested here"
//@ diag: 46:15 note "in instantiation of function template specialization 'bind_site<Widget>' requested here"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:54 note "unhandled exception: widget: SECOND_REFUSAL"
//@ diag: 52:17 error "static assertion failed due to requirement 'sizeof(Widget) == 0': gadget: FIRST_REFUSAL"
//@ diag: 56:31 note "in instantiation of function template specialization 'pick<Widget>' requested here"
//@ diag: 59:10 note "in instantiation of variable template specialization 'pick_v<Widget>' requested here"
//@ diag: 61:15 note "in instantiation of function template specialization 'bind_pick<Widget>' requested here"
//@ diag: 52:27 note "expression evaluates to '1 == 0'"
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:54 note "unhandled exception: widget: SECOND_REFUSAL"
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

// A plain variable template initializer that fails at instantiation.
template <class T>
inline constexpr int rule_v = T::missing;
template <class T>
consteval bool bind_site() {
  (void)rule_v<T>;
  return second_refusal();
}
struct Widget {};
static_assert(bind_site<Widget>());

// A plain variable template initialized by a call whose function fails
// a static_assert: the value is still read.
template <class T>
consteval int pick() {
  static_assert(sizeof(T) == 0, "gadget: FIRST_REFUSAL");
  return 1;
}
template <class T>
inline constexpr int pick_v = pick<T>();
template <class T>
consteval bool bind_pick() {
  return pick_v<T> == 1 && second_refusal();
}
static_assert(bind_pick<Widget>());
