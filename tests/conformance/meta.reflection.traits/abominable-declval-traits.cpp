// [declval], [dcl.fct]/6, [meta.unary.prop] is_assignable,
// is_constructible, [meta.rel] is_convertible, [meta.unary.prop]
// reference_constructs_from_temporary: every trait defined by an
// expression on `declval<U>()` is false when U is an abominable function
// type (cv- or ref-qualified: `void() const`, `void() &`, `void() const
// noexcept`), because `declval<U>()` cannot be formed (its return type
// would be the function type itself), even for a class constructible and
// assignable from anything; an unqualified function type `void()` or
// `void() noexcept` gives a function lvalue and the traits hold. clang
// 22 answers true for the qualified forms; GCC 16.1 answers false except
// for `__is_constructible`; libc++'s is_constructible tests and the text
// say false. WP-1613.
//@ skip(clang): clang 22 builds the operation on an lvalue of an abominable function type
//@ skip(gcc): GCC 16.1 answers true for __is_constructible of an abominable function type
//@ expect: compile-ok
struct All {
  All();
  template <class T> All(T &&);
  template <class T> All &operator=(T &&);
};
using F = void() const;
using G = void() &;
using H = void() const noexcept;
using P = void();
using Q = void() noexcept;
static_assert(!__is_assignable(All &, F));
static_assert(!__is_nothrow_assignable(All &, G));
static_assert(!__is_trivially_assignable(All &, H));
static_assert(!__is_constructible(All, F));
static_assert(!__is_nothrow_constructible(All, int, G));
static_assert(!__is_trivially_constructible(All, H));
static_assert(!__is_convertible(F, All));
static_assert(!__is_convertible_to(G, All));
static_assert(!__is_nothrow_convertible(H, All));
static_assert(!__reference_constructs_from_temporary(All &&, F));
static_assert(!__reference_converts_from_temporary(const All &, G));
static_assert(!__is_constructible(F));
static_assert(!__is_constructible(F, F));
// The accepted side: an unqualified function type is referenceable.
static_assert(__is_assignable(All &, P));
static_assert(__is_assignable(All &, Q));
static_assert(__is_constructible(All, P));
static_assert(__is_constructible(All, Q &));
static_assert(__is_convertible(P, All));
static_assert(__is_constructible(P &, P));
static_assert(__is_convertible(P, P &));
