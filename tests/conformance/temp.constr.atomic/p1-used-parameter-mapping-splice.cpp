// [temp.constr.atomic]/1, [temp.constr.order] with P2996R13 splices: an
// atomic constraint's parameter mapping covers the template parameters
// named in a splice-scope-specifier's, a splice-type-specifier's and a
// splice-expression's operand (`[: R :]::value`, `typename [: R :]`,
// `[: R :]`), and only those, so `C<^^T, int>` and `C<^^T, long>` give
// the same atom and the overload adding a conjunct is more constrained
// (an over-wide mapping would make each call ambiguous). WP-776.
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
#include "kiln_meta.h"
struct V { static constexpr int value = 1; int m; };

template <std::meta::info R, class U> concept Scope = [: R :]::value > 0;
template <class T> requires Scope<^^T, int> constexpr int f1(T) { return 1; }
template <class T> requires Scope<^^T, long> && true constexpr int f1(T) { return 2; }
static_assert(f1(V{}) == 2);

template <std::meta::info R, class U> concept Type = sizeof(typename [: R :]) > 0;
template <class T> requires Type<^^T, int> constexpr int f2(T) { return 1; }
template <class T> requires Type<^^T, long> && true constexpr int f2(T) { return 2; }
static_assert(f2(V{}) == 2);

template <std::meta::info R, class U> concept Expr = [: R :] > 0;
template <class T> requires Expr<^^V::value, int> constexpr int f3(T) { return 1; }
template <class T> requires Expr<^^V::value, long> && true constexpr int f3(T) { return 2; }
static_assert(f3(V{}) == 2);

// `^^T` of a template type parameter is equivalent across the two
// templates ([temp.over.link]/5), so the mappings of `C<^^T>` match.
template <std::meta::info R> concept Refl = sizeof(typename [: R :]) == sizeof(V);
template <class T> requires Refl<^^T> constexpr int f4(T) { return 1; }
template <class T> requires Refl<^^T> && true constexpr int f4(T) { return 2; }
static_assert(f4(V{}) == 2);
