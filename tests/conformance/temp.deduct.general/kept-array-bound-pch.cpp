// kept-array-bound-sfinae.cpp through a precompiled header: the
// snapshot carries a constant array's kept bound, the bounds that name
// a pack, and an adjusted parameter's `Decayed` type (spec/types.md §7.2
// "Kept bounds", WP-2720), so the including file still substitutes them.
//@ pch: Inputs/kept_array_bound_pch.h
//@ std: c++26
//@ expect: compile-ok
static_assert(g(X(), nullptr) == 2);
static_assert(g(1, nullptr) == 1);
static_assert(k(X(), nullptr) == 2);
static_assert(k(1, nullptr) == 1);
static_assert(__is_same(S<int, long>::t, L<char[sizeof(sizeof(0))], char[sizeof(sizeof(0))]>));
