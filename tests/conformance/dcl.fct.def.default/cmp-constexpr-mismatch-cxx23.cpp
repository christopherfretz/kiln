// [dcl.fct.def.default]/3 (C++23, P2448R2): a defaulted comparison may be
// declared constexpr or consteval even if it invokes a non-constexpr
// comparison function (cmp-constexpr-mismatch.cpp is the C++20 error).
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
#include <compare>
struct NC {
  bool operator==(const NC&) const { return true; }
  std::strong_ordering operator<=>(const NC&) const { return std::strong_ordering::equal; }
};
struct Mem { int x; NC m; constexpr bool operator==(const Mem&) const = default; };
struct Base : NC { constexpr bool operator==(const Base&) const = default; };
struct Friend { NC m; constexpr friend bool operator==(const Friend&, const Friend&) = default; };
struct Eval { NC m; consteval bool operator==(const Eval&) const = default; };
struct Three { NC m; constexpr std::strong_ordering operator<=>(const Three&) const = default; };
struct Implicit { NC m; constexpr auto operator<=>(const Implicit&) const = default; };
struct Ordered {
  std::strong_ordering operator<=>(const Ordered&) const = default;
  NC m;
  constexpr bool operator<(const Ordered&) const = default;
};
struct NotEq { NC m; constexpr bool operator!=(const NotEq&) const = default; bool operator==(const NotEq&) const = default; };
template<class T> struct Tmpl { T m; constexpr bool operator==(const Tmpl&) const = default; };
Tmpl<NC> t;
bool b = t == t;
struct Fine { int x; constexpr bool operator==(const Fine&) const = default; };
static_assert(Fine{1} == Fine{1});
