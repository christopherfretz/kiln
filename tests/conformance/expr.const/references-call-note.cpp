// [expr.const]/5.8, [dcl.init.ref]: a call note prints a reference argument
// as its referent (clang: `bad(a[1], 0)`, not `bad(&a[1], 0)`), and a read
// through a reference bound to `*nullptr` is diagnosed at the read.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
//@ diag: 13:51 note "division by zero"
//@ diag: 14:50 note "in call to 'bad(a[1], 0)'"
//@ diag: 15:15 note "in call to 'f()'"
//@ diag: 17:15 error "static assertion expression is not an integral constant expression"
//@ diag: 16:62 note "read of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 17:15 note "in call to 'dang()'"
constexpr int bad(const int &x, int y) { return x / y; }
constexpr int f() { int a[3] = {1, 2, 3}; return bad(a[1], 0); }
static_assert(f() == 1);
constexpr int dang() { int *p = nullptr; int &r = *p; return r; }
static_assert(dang() == 0);
