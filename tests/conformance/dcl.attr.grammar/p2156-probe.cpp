// [dcl.attr.grammar]/5 (C++23, P2156R1): a standard attribute may appear
// more than once in an attribute-list. WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
[[nodiscard, nodiscard]] int f();
[[noreturn, noreturn]] void g();
[[deprecated, deprecated]] void h();
[[maybe_unused, maybe_unused]] static int i;
