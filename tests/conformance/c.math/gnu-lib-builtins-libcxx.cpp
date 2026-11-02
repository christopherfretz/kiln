// [constexpr.functions]/1, [expr.const]/5.3: under -stdlib=libc++ kiln
// recognizes no GCC implicit library builtin (clang's behaviour; libc++'s
// constexpr-cxx23-clang.pass.cpp requires `::ceil` to stay non-constant):
// a call of the C library `ceil` is not a constant expression
// (spec/sema.md §5.4, WP-380).
//@ expect: compile-fail
//@ strict-diags
//@ args: -stdlib=libc++
//@ diag: 17:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:15 note "non-constexpr function 'ceil' cannot be used in a constant expression"
//@ diag: 15:19 note "declared here"
//@ diag: 18:18 error "constexpr variable 'f' must be initialized by a constant expression"
//@ diag: 18:22 note "non-constexpr function 'fabs' cannot be used in a constant expression"
//@ diag: 16:19 note "declared here"
extern "C" double ceil(double);
extern "C" double fabs(double);
static_assert(ceil(1.5) == 2.0);
constexpr double f = fabs(-1.0);
