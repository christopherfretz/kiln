// [expr.const]: clang 22 constant-evaluates the integer element-wise
// builtins on a scalar _BitInt wider than 128 bits as on any integer
// (abs wraps on the most negative value; add_sat / sub_sat saturate).
// x86-64 only (aarch64's __BITINT_MAXWIDTH__ is 128).
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu
//@ xfail(kiln): WP-1741 remainder: the element-wise builtins on a wide _BitInt need its in-memory result (compile/wide.rs)
using W = _BitInt(200);
using U = unsigned _BitInt(130);
constexpr W w = -5;
static_assert(__builtin_elementwise_abs(w) == 5);
static_assert(__builtin_elementwise_max(w, W(3)) == 3);
static_assert(__builtin_elementwise_min(w, W(3)) == -5);
static_assert(__builtin_elementwise_add_sat(w, W(7)) == 2);
static_assert(__builtin_elementwise_sub_sat(U(1), U(3)) == 0);
static_assert(__builtin_elementwise_add_sat(~U(0), U(1)) == ~U(0));
constexpr W wmin = W(-1) << 199;
static_assert(__builtin_elementwise_sub_sat(wmin, W(1)) == wmin);
