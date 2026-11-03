// GNU `init_priority(N)` range check, clang 22 wording (WP-189): above
// 65535 is an error, below 101 is the default-error warning
// -Winit-priority-reserved (the attribute is kept), and the arg-count
// error names the scoped spelling. [dcl.attr.grammar]/5; the clang 19
// wording is in gnu-init-priority-range-clang19.cpp.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:22 error "requested 'init_priority' 0 is reserved for internal use [-Winit-priority-reserved]"
//@ diag: 15:24 error "requested 'init_priority' 100 is reserved for internal use [-Winit-priority-reserved]"
//@ diag: 18:21 error "'init_priority' attribute requires integer constant between 0 and 65535 inclusive"
//@ diag: 19:3 error "'gnu::init_priority' attribute takes one argument"
struct S { S(); };
S lo0 __attribute__((init_priority(0)));
S lo100 __attribute__((init_priority(100)));
S ok101 __attribute__((init_priority(101)));
S ok65535 __attribute__((init_priority(65535)));
S hi __attribute__((init_priority(65536)));
[[gnu::init_priority(1, 2)]] S two;
