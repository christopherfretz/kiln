// [dcl.pre]/12 (P2741R3): when the condition of a static_assert-declaration
// is true, or the declaration is in a template definition, it has no
// effect and its static_assert-message is an unevaluated operand. A
// message that is a valid object with usable `size()` and `data()` but is
// not a constant expression (a consteval call that throws, a null `data()`,
// a null dereference in `size()`, a non-constexpr `size()`) is fine, and so
// is a consteval call there (not an immediate invocation, [expr.const]).
// The templated assertions have value-dependent conditions that hold on
// instantiation. clang 19 and 22 evaluate the message anyway
// (-Winvalid-static-assert-message, an error by default): kiln follows the
// standard and GCC 16.1 (WP-570; spec/sema.md §4.6).
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
//@ xfail(clang): clang evaluates the message of an assertion that holds (-Winvalid-static-assert-message is an error by default)
struct View {
  const char* p;
  unsigned long n;
  constexpr const char* data() const { return p; }
  constexpr unsigned long size() const { return n; }
};
struct Rt {
  unsigned long size() const { return 1; }
  const char* data() const { return "x"; }
};
consteval View thrower() { throw 1; }
constexpr int deref(const int* p) { return *p; }
constexpr View null_data() { return View{nullptr, 1}; }
constexpr View null_size() { return View{"x", static_cast<unsigned long>(deref(nullptr))}; }
static_assert(true, thrower());
static_assert(sizeof(int) > 0, null_data());
static_assert(1 + 1 == 2, null_size());
static_assert(true, Rt{});
template <int N> void f() {
  static_assert(N > 0, thrower());
  static_assert(N > 0, null_data());
  static_assert(N == 1, Rt{});
  static_assert(true, thrower());
}
template <class T, class M> struct Holds {
  static_assert(sizeof(T) > 0, M{});
  static_assert(sizeof(T) >= 1, thrower());
};
// Never instantiated: the condition is not even evaluated.
template <class T> void never() { static_assert(false, thrower()); }
template void f<1>();
Holds<int, Rt> h1;
Holds<char, View> h2;
int main() {}
