// The trivially copyable / trivial matrix (WP-1452), part 3: rows where
// GCC 16.1 follows the text and clang 19/22 do not.
// - [class.prop]/2, [special]/6: a deleted default constructor is not
//   eligible, so a class whose only default constructor is deleted (a
//   const scalar or reference member without initializer,
//   [class.default.ctor]/2; `T() = delete`) is trivially copyable but not
//   trivial, and not POD. clang counts it trivial.
// - [class.prop]/1: a deleted assignment operator (a const member of
//   class type) or a deleted variadic copy constructor is not eligible
//   and does not make the class non-trivially copyable. clang counts it.
//@ oracle: gcc
//@ skip(clang): clang 19/22 count deleted special members as eligible and non-trivial or trivial
//@ strict-diags
#include "Inputs/trivially-copyable-classes.h"

// TC: __is_trivially_copyable(T) ([class.prop]/1)
static_assert(TC(C1)); static_assert(TC(CVM)); static_assert(TC(VarDel));

// TRIV: __is_trivial(T) ([class.prop]/2)
static_assert(!TRIV(T1)); static_assert(TRIV(C1)); static_assert(!TRIV(R));
static_assert(!TRIV(RR)); static_assert(!TRIV(DefDel));
static_assert(!TRIV(DB));

// POD: __is_pod(T) ([depr.meta.types]/4)
static_assert(!POD(T1)); static_assert(POD(C1)); static_assert(!POD(DefDel));
static_assert(!POD(DB));

// A deleted variadic copy constructor is not eligible; the implicit copy
// assignment operator is ([class.prop]/1, /2). `AD(int = 0)` is an
// eligible non-trivial default constructor, so `AD` is neither trivial
// nor POD; clang reads only `AD() = default` for POD.
static_assert(TC(KV)); static_assert(TRIV(KV)); static_assert(POD(KV));
static_assert(!TRIV(AD)); static_assert(!POD(AD));
