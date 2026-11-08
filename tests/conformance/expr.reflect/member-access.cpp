// [expr.reflect]/7 (P2996R13), [class.access]/2, [class.friend]/2: `^^m`
// and `^^C::m` of a private member are well-formed inside the class (a
// member function body, a static data member initializer), in a friend,
// and for a public member anywhere; `^^T::pub` instantiated likewise.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta_queries.h"
#endif
class C {
  int priv;
  friend struct F;
public:
  int pub;
  static constexpr auto inside = ^^priv;
  static consteval auto get() { return ^^C::priv; }
};
struct F { static constexpr auto r = ^^C::priv; };
constexpr auto ok = ^^C::pub;
static_assert(C::get() == C::inside && F::r == C::inside);
static_assert(std::meta::identifier_of(C::get()) == "priv");
template <class T> consteval auto dep() { return ^^T::pub; }
static_assert(dep<C>() == ok);
