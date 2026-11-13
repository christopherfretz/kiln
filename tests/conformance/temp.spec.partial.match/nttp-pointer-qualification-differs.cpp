// [temp.deduct.type]/20, [temp.spec.partial.match]/2: for `C<(const int *)&x>`
// the primary's `auto` parameter takes the type `const int *`, which
// differs from the type `int *` of the partial specialization's `P`, so
// it does not match (no qualification conversion is applied). Clang 19/22
// agree; GCC 16.1 matches it. Split from nttp-deduced-value-type-run.cpp
// (WP-1405).
//@ std: c++20
//@ expect: compile-ok
int x;
template <auto V> struct C { static constexpr int v = 0; };
template <int *P> struct C<P> { static constexpr int v = 1; };
static_assert(C<&x>::v == 1);
static_assert(C<(const int *)&x>::v == 0);
