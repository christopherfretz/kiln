// [c.math.syn], [expr.const]/5.3: under -stdlib=libstdc++ a call of an
// implicit library builtin folds only when the function is clang's
// library builtin: a namespace-scope declaration with C language linkage
// and the C library type (spec/sema.md §5.4, WP-380). rint/nearbyint are
// not folded (GCC does not fold them: their result depends on the
// rounding mode), nor is a C++-linkage `trunc` or a namespace member.
// Reference: g++ 14 gives the same accept/reject split.
//@ expect: compile-fail
//@ strict-diags
//@ args: -stdlib=libstdc++
//@ skip(clang): GCC is the reference; clang folds no C library function
//@ diag: 26:18 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 26:22 note "non-constexpr function 'rint' cannot be used in a constant expression"
//@ diag: 18:19 note "declared here"
//@ diag: 27:18 error "constexpr variable 'n' must be initialized by a constant expression"
//@ diag: 27:22 note "non-constexpr function 'floor' cannot be used in a constant expression"
//@ diag: 21:22 note "declared here"
extern "C" double rint(double);
extern "C" double ceil(double);
extern "C" { double fmin(double, double) noexcept; }
namespace m { double floor(double); }

static_assert(ceil(1.5) == 2.0);
static_assert(fmin(-0.0, 1.0) == -0.0);
constexpr double c = ceil(-1.5);
constexpr double r = rint(1.5);
constexpr double n = m::floor(1.5);
