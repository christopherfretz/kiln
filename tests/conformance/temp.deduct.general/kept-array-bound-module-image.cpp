// kept-array-bound-sfinae.cpp and adjusted-array-param-bound-substituted.cpp
// through a module's declaration image (`-fkiln-bmi-image`): the image
// carries a constant array's kept bound and an adjusted parameter's
// `Decayed` type (spec/types.md §7.2 "Kept bounds", WP-2720), so the
// importer still substitutes them.
//@ module: kept_array_bound_mod=Inputs/kept_array_bound_mod.cppm
//@ two-step
//@ args: -fkiln-bmi-image
//@ skip(clang): kiln-only flags
//@ std: c++26
//@ expect: compile-ok
import kept_array_bound_mod;
static_assert(g(X(), nullptr) == 2);
static_assert(g(1, nullptr) == 1);
static_assert(k(X(), nullptr) == 2);
static_assert(k(1, nullptr) == 1);
static_assert(n(X(), nullptr) == 2);
static_assert(n(1, nullptr) == 1);
static_assert(__is_same(S<int, long>::t, L<char[sizeof(sizeof(0))], char[sizeof(sizeof(0))]>));
