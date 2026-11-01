// [cpp.cond]/13 (C++23, P2334R1): `#elifdef` and `#elifndef`. Lines in
// skipped groups end in `_wrong` (the cpp.cond marker convention of
// crates/kiln_pp/tests/ifeval_conformance.rs). WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
#define A
#ifdef NOT_DEFINED
int ifdef_wrong;
#elifdef A
constexpr int x = 1;
#else
int else_wrong;
#endif
#if 0
int if0_wrong;
#elifndef NOT_DEFINED
constexpr int y = 2;
#endif
static_assert(x + y == 3);
