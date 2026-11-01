// [cpp.cond]/13 (C++23, P2334R1): `#elifdef` and `#elifndef`.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
#define A
#ifdef NOT_DEFINED
static_assert(false);
#elifdef A
constexpr int x = 1;
#else
static_assert(false);
#endif
#if 0
static_assert(false);
#elifndef NOT_DEFINED
constexpr int y = 2;
#endif
static_assert(x + y == 3);
