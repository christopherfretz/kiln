// [dcl.struct.bind]/4, [expr.const]: a `constexpr` tuple-like structured
// binding's holding variables are usable in constant expressions when
// `get<i>` returns a prvalue, but the decomposed temporary `e` is not: it
// is not const, so a `get` that reads it is not a constant expression.
// The accepted side: constexpr-prvalue-get.cpp.
//@ skip(clang): clang 22 does not implement P2686R5 constexpr structured bindings
//@ skip(gcc): GCC 16.1 does not implement P2686R5 constexpr references to automatic objects
//@ expect: compile-fail
//@ diag: 20:21 error "constexpr variable 'a' must be initialized by a constant expression"
namespace std {
template <class> struct tuple_size;
template <decltype(sizeof 0), class> struct tuple_element;
} // namespace std
struct Q { int v = 3; };
template <> struct std::tuple_size<Q> { static constexpr int value = 1; };
template <decltype(sizeof 0) I> struct std::tuple_element<I, Q> { using type = int; };
template <decltype(sizeof 0) I> constexpr int get(const Q &q) { return q.v; }

int f() {
  constexpr auto &&[a] = Q{};
  return a;
}
