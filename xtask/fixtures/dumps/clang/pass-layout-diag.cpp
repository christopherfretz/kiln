//@ expect: layout
//@ diag: 4:17 warning "unknown attribute 'unused' ignored"
struct S { int i; };
[[maybe_unused, unused]] static int unused_x;
