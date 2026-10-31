// [expr.spaceship]/4-8, [cmp.categories]: builtin `<=>` on arithmetic,
// enumeration and pointer operands yields the standard comparison
// category type; constant evaluation of its result.
//@ expect: compile-ok
//@ strict-diags
#include "Inputs/compare.h"
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
static_assert(same<decltype(1 <=> 2), std::strong_ordering>::value);
static_assert(same<decltype(1u <=> 2u), std::strong_ordering>::value);
static_assert(same<decltype(1.0 <=> 2), std::partial_ordering>::value);
static_assert(same<decltype(1.0f <=> 2.0), std::partial_ordering>::value);
enum E { e0, e1 };
enum class F : long { f0, f1 };
static_assert(same<decltype(e0 <=> e1), std::strong_ordering>::value);
static_assert(same<decltype(F::f0 <=> F::f1), std::strong_ordering>::value);
int arr[3];
static_assert(same<decltype(&arr[0] <=> &arr[1]), std::strong_ordering>::value);
static_assert((1 <=> 2) < 0);
static_assert((2 <=> 2) == 0);
static_assert((3 <=> 2) > 0);
static_assert((1 <=> 2) == std::strong_ordering::less);
static_assert((1.5 <=> 1.5) == std::partial_ordering::equivalent);
static_assert((0.5 <=> 1) == std::partial_ordering::less);
static_assert((e1 <=> e0) == std::strong_ordering::greater);
static_assert((F::f0 <=> F::f1) < 0);
static_assert((&arr[0] <=> &arr[2]) < 0);
static_assert((&arr[1] <=> &arr[1]) == 0);
constexpr double nan = __builtin_nan("");
static_assert((nan <=> 1.0) == std::partial_ordering::unordered);
static_assert(!((nan <=> nan) == 0));
// The result converts to the weaker categories.
constexpr std::weak_ordering w = 1 <=> 2;
constexpr std::partial_ordering p = 3 <=> 2;
static_assert(w < 0 && p > 0);
