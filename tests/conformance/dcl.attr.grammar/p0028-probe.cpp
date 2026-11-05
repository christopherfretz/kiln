// [dcl.attr.grammar]/3 (C++17, P0028R4): an attribute-using-prefix applies
// a namespace to every attribute in the list. WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
[[using gnu: always_inline, hot]] inline int f() { return 1; }
[[using gnu: noinline, cold]] int g() { return 2; }
struct [[using gnu: packed]] P { char c; int i; };
static_assert(sizeof(P) == 5);
[[using gnu: aligned(16)]] int a;
int h() { return f() + g(); }
