// [expr.const]/10.8 (clang's VLA extension): the size of a variable length
// array is a run-time value, so no constant evaluation uses it. Pointer
// arithmetic on a pointer to a VLA and a subscript yielding a VLA are
// "subexpression not valid" at the expression's location; naming,
// loading, decaying and comparing such a pointer needs no size and stays
// constant (h6), as in clang.
//@ args: -Wno-vla-cxx-extension
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:47 note "subexpression not valid in a constant expression"
//@ diag: 23:15 note "in call to 'h5(2)'"
//@ diag: 27:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:61 note "subexpression not valid in a constant expression"
//@ diag: 27:15 note "in call to 'h7(2)'"
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:52 note "subexpression not valid in a constant expression"
//@ diag: 29:15 note "in call to 'h8(2)'"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:54 note "subexpression not valid in a constant expression"
//@ diag: 31:15 note "in call to 'h9(2)'"
constexpr int h5(int n) { int a[n][n]; return a[1][0]; }
static_assert(h5(2) == 0, "");
constexpr int h6(int n) { int (*p)[n] = nullptr; return p == nullptr; }
static_assert(h6(2), "");
constexpr int h7(int n) { int a[n][n]; auto p = a; return p + 1 != p; }
static_assert(h7(2), "");
constexpr int h8(int n) { int a[n][n]; auto p = a; ++p; return 1; }
static_assert(h8(2), "");
constexpr int h9(int n) { int a[n][n]; auto p = a; p += 1; return 1; }
static_assert(h9(2), "");
