// The trivially copyable / trivial matrix (WP-1452), part 4: rows where
// neither clang 19/22 nor GCC 16.1 follows the text.
// - [class.prop]/1: a class whose copy and move members are all deleted
//   (explicitly, through a base or member, or ineligible through a more
//   constrained deleted one, [special]/6) has no eligible one and is not
//   trivially copyable; neither is a class with a deleted destructor.
//   Both count the deleted members as trivial.
// - [class.prop]/2: `K3<true>`'s only eligible-by-constraints default
//   constructor is deleted, so it has no eligible one; `N`'s deleted
//   `N(...)` is not eligible, its `N() = default` is, so `N` is trivial.
// - `U2` (C++26, P3074R7): a union whose variant member has a non-trivial
//   copy constructor has deleted copy/move constructors but trivial
//   assignments, so it is trivially copyable, and its default
//   constructor is trivial (spec/sema-class.md §4.2), so it is trivial.
//@ skip(clang): clang 19/22 count deleted special members as eligible
//@ skip(gcc): GCC 16.1 counts deleted special members as eligible
//@ strict-diags
#include "Inputs/trivially-copyable-classes.h"

// TC: __is_trivially_copyable(T) ([class.prop]/1)
static_assert(!TC(AllDel)); static_assert(!TC(AllDel4));
static_assert(!TC(DelDtor)); static_assert(TC(U2));
static_assert(!TC(K2<true>)); static_assert(!TC(B9)); static_assert(!TC(M9));
static_assert(!TC(OnlyMoveDel));

// TRIV: __is_trivial(T) ([class.prop]/2)
static_assert(!TRIV(AllDel4)); static_assert(!TRIV(DelDtor));
static_assert(TRIV(U2)); static_assert(!TRIV(K3<true>));
static_assert(!TRIV(B9)); static_assert(!TRIV(M9)); static_assert(TRIV(N));

// POD: __is_pod(T) ([depr.meta.types]/4)
static_assert(!POD(AllDel4)); static_assert(!POD(DelDtor));
static_assert(POD(U2)); static_assert(!POD(K3<true>)); static_assert(!POD(B9));
static_assert(!POD(M9));
