// [temp.constr.atomic]/3, [support.srcloc.cons]/2: a function template's
// constraints are checked from the function (as clang's
// CheckFunctionConstraints does, so its access applies), and a call in
// them to a function whose default argument is
// `std::source_location::current()` is a complete expression once the
// template arguments are substituted: the default argument's value does
// not leave the atomic constraint value-dependent. Reduced from an
// awaitable-transform requirement in a coroutine library (WP-1325).
//@ expect: run
#include <source_location>
struct Aw { bool await_ready(); };
struct Promise {
  template <typename T>
  Aw await_transform(T &&t, const std::source_location &loc = std::source_location::current());
};
template <typename T>
  requires requires(Promise &pro, T &&aw) { pro.await_transform(static_cast<T &&>(aw)).await_ready(); }
int await(T &&) { return 1; }
int await(...) { return 2; }
struct Op {};
template <class U>
constexpr const char *name(U, const char *n = __builtin_FUNCTION()) { return n; }
template <char C> struct X { static constexpr bool value = C != '?'; };
// The value of a source-location default argument as a template argument.
template <class T>
  requires(X<name(T{})[0]>::value)
int arg(T) { return 1; }
int arg(...) { return 2; }
int main() { return await(Op{}) == 1 && arg(0) == 1 ? 0 : 1; }
