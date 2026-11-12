// [over.match.oper]/3.4, [over.match.best.general]/2, [temp.deduct.general]/8,
// [expr.prim.req.general]/5, [temp.constr.atomic]/3: a call whose best
// non-reversed candidate ties with the reversed form of a candidate
// (`Derived::operator<=>(const Base&)` for `a < b`, the pre-C++20
// `operator==(const Base&)` for `a == b` and `a != b`) is ambiguous in
// C++20. Outside SFINAE clang and GCC accept it with a warning (kiln's
// extension, -Wambiguous-reversed-operator); in a SFINAE context it is a
// substitution failure, also when the warning is disabled: a
// requires-expression, `decltype` in a trailing return type or in an
// `enable_if` default template argument, a concept's atomic constraint
// and `if constexpr (requires ...)` all see the operator as unusable. A
// class whose comparisons do not tie is unaffected. WP-1322 (the
// reference workload): kiln satisfied the requirements.
//@ expect: run
//@ std: c++20
//@ args: -Wno-ambiguous-reversed-operator
//@ stdout: 0 0 0 1
//@ stdout: 0 0 1 1
//@ stdout: 0 1 0 1
//@ stdout: 0 0 1 1
//@ stdout: 1 1
#include <compare>
#include <concepts>
#include <cstdio>
#include <type_traits>
#include <utility>
struct Base { int v = 0; };
struct Derived : Base {
  std::strong_ordering operator<=>(const Base &o) const { return v <=> o.v; }
};
struct EqB { int v = 0; };
struct EqD : EqB { bool operator==(const EqB &o) const { return v == o.v; } };
struct Fine {
  int v = 0;
  bool operator==(const Fine &) const = default;
  auto operator<=>(const Fine &) const = default;
};
template <class T>
constexpr bool has_lt = requires(const T &a, const T &b) { a < b; };
template <class T>
constexpr bool has_eq = requires(const T &a, const T &b) { a == b; };
template <class T>
constexpr bool has_ne = requires(const T &a, const T &b) { a != b; };
template <class T>
concept LessComparable = requires(const T &a, const T &b) {
  { a < b } -> std::convertible_to<bool>;
};
template <class T>
concept EqAtomic = std::is_same_v<
    decltype(std::declval<const T &>() == std::declval<const T &>()), bool>;
template <class T>
auto lt(const T &a, const T &b, int) -> decltype(a < b, 1) { return 1; }
template <class T> int lt(const T &, const T &, long) { return 0; }
template <class T, class = std::enable_if_t<std::is_same_v<
                       decltype(std::declval<const T &>() !=
                                std::declval<const T &>()),
                       bool>>>
int ne(int) { return 1; }
template <class T> int ne(long) { return 0; }
template <class T> int ifc() {
  if constexpr (requires(const T &a) { a < a; })
    return 1;
  else
    return 0;
}
template <class T> int conc() { return 0; }
template <LessComparable T> int conc() { return 1; }
template <class T> int conc2() { return 0; }
template <class T> requires EqAtomic<T> int conc2() { return 1; }

bool direct(const Derived &a, const Derived &b) { return a <= b; }
bool direct_eq(const EqD &a, const EqD &b) { return a == b; }

int main() {
  Derived d;
  EqD e;
  Fine f;
  std::printf("%d %d %d %d\n", has_lt<Derived>, has_eq<EqD>, has_ne<EqD>,
              has_lt<Fine>);
  std::printf("%d %d %d %d\n", lt(d, d, 0), ne<EqD>(0), lt(f, f, 0),
              ne<Fine>(0));
  std::printf("%d %d %d %d\n", ifc<Derived>(), ifc<Fine>(), conc<Derived>(),
              conc<Fine>());
  std::printf("%d %d %d %d\n", conc2<EqD>(), conc2<Derived>(), conc2<Fine>(),
              has_eq<Fine>);
  std::printf("%d %d\n", direct(d, d), direct_eq(e, e));
}
