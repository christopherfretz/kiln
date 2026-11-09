// [expr.const]/10, [stmt.break]/1, [stmt.cont]/1 (GNU statement expressions):
// a constexpr function may contain a statement expression in a loop or
// switch header whose break or continue leaves it, but constant evaluation
// rejects such a jump, with clang's note at the statement of the statement
// expression that holds the jump. A loop nested wholly inside a statement
// expression evaluates normally (ok). The same functions run correctly at
// run time (stmt.break/stmt-expr-loop-header-run.cpp). WP-1075;
// spec/sema.md §7.4a.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:43 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 37:15 note "in call to 'cw()'"
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:55 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 38:15 note "in call to 'cf()'"
//@ diag: 39:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:74 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 39:15 note "in call to 'cs()'"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:67 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 40:15 note "in call to 'cd()'"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:44 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 41:15 note "in call to 'ci()'"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:93 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 42:15 note "in call to 'cr()'"

constexpr int cw() { int n = 0; while (({ if (n == 2) break; true; })) ++n; return n; }
constexpr int cf() { int n = 0, c = 0; for (; ({ ++c; if (c < 3) continue; n < 5; }); ++n) {} return n * 100 + c; }
constexpr int cs() { int r = 0; for (int o = 0; o < 3; ++o) { switch (({ if (o == 1) continue; o; })) { case 0: r += 1; break; default: r += 100; } r += 10; } return r; }
constexpr int cd() { int n = 0, c = 0; do { ++n; } while (({ ++c; if (c < 3) continue; n < 5; })); return n * 100 + c; }
constexpr int ci() { int n = 0; for (;; ({ if (n == 3) break; ++n; })) {} return n; }
constexpr int cr() { int a[2] = {1, 2}, r = 0; for (int o = 0; o < 2; ++o) for (int x : *({ if (o == 1) break; &a; })) r += x; return r; }
constexpr int ok() { int n = 0; while (({ int k = 0; for (;;) { if (++k == 2) break; } n < k + 3; })) ++n; return n; }
static_assert(cw() == 2);
static_assert(cf() == 506);
static_assert(cs() == 121);
static_assert(cd() == 507);
static_assert(ci() == 3);
static_assert(cr() == 3);
static_assert(ok() == 5);
