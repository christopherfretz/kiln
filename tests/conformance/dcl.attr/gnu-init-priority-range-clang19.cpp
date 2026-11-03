// GNU `init_priority(N)` range check, clang 19 wording (WP-189): outside
// 101..=65535 is an error and the arg-count error names the unscoped
// spelling. [dcl.attr.grammar]/5; the clang 22 wording is in
// gnu-init-priority-range.cpp.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:22 error "'init_priority' attribute requires integer constant between 101 and 65535 inclusive"
//@ diag: 14:24 error "'init_priority' attribute requires integer constant between 101 and 65535 inclusive"
//@ diag: 17:21 error "'init_priority' attribute requires integer constant between 101 and 65535 inclusive"
//@ diag: 18:3 error "'init_priority' attribute takes one argument"
struct S { S(); };
S lo0 __attribute__((init_priority(0)));
S lo100 __attribute__((init_priority(100)));
S ok101 __attribute__((init_priority(101)));
S ok65535 __attribute__((init_priority(65535)));
S hi __attribute__((init_priority(65536)));
[[gnu::init_priority(1, 2)]] S two;
