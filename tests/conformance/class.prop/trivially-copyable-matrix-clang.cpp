// The trivially copyable / trivial matrix (WP-1452), part 2: rows where
// clang 19/22 follow the text and GCC 16.1 does not. A `volatile` (or
// `const volatile`) member of class type `I` deletes all four copy/move
// members ([class.copy.ctor]/10, [class.copy.assign]/7: no constructor or
// assignment of `I` takes a volatile `I`), so none is eligible
// ([special]/6) and the class is not trivially copyable ([class.prop]/1),
// nor is a class with a member, base or array of such a class. GCC 16.1
// counts the deleted members as trivial (docs/status/deviations.md).
// `UC` (C++26, P3074R7 removed the all-const-union bullet of
// [class.default.ctor]/2) and `N` are trivial and POD by the text; clang
// agrees for its own reason (it counts the deleted members as trivial).
//@ skip(gcc): GCC 16.1 counts deleted copy and move members as eligible
//@ strict-diags
#include "Inputs/trivially-copyable-classes.h"

// TC: __is_trivially_copyable(T) ([class.prop]/1)
static_assert(!TC(C5)); static_assert(!TC(CV)); static_assert(!TC(Arr));
static_assert(!TC(Arr1)); static_assert(!TC(VArr));
static_assert(!TC(DefDelCopy)); static_assert(!TC(CB)); static_assert(!TC(UV));

// TRIV: __is_trivial(T) ([class.prop]/2)
static_assert(!TRIV(C5)); static_assert(!TRIV(Arr)); static_assert(TRIV(UC));
static_assert(!TRIV(Arr1)); static_assert(!TRIV(VArr));
static_assert(!TRIV(CB)); static_assert(!TRIV(UV));

// POD: __is_pod(T) ([depr.meta.types]/4)
static_assert(!POD(C5)); static_assert(!POD(Arr)); static_assert(POD(UC));
static_assert(POD(N)); static_assert(!POD(Arr1)); static_assert(!POD(VArr));
static_assert(!POD(CB)); static_assert(!POD(UV));

// TDC: __is_trivially_constructible(T) ([meta.unary.prop])
static_assert(!TDC(K3<true>));

// TCC: __is_trivially_constructible(T, const T&) ([meta.unary.prop])
static_assert(!TCC(K2<true>));

// ILT: __builtin_is_implicit_lifetime(T) ([class.prop]/9)
#if __has_builtin(__builtin_is_implicit_lifetime)
static_assert(!ILT(K2<true>));
#endif
