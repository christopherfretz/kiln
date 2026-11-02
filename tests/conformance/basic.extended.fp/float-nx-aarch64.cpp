// [basic.extended.fp], [conv.rank]/2, [expr.arith.conv]/1.4: on AArch64
// GCC's _Float64x is binary128, long double's format, and has _Float128's
// value set: _Float64x <-> _Float128 converts without a warning, and mixed
// arithmetic yields _Float128 (GCC prefers _FloatN over _FloatNx of equal
// value set). The __FLT64X_*__ values are aarch64-linux-gnu-g++-14 -dM -E's.
// Reference: GCC 14 (WP-380).
//@ expect: compile-ok
//@ std: c++23
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ skip(clang): clang 19 and 22 have no _Float64x (GCC is the reference)
#if __FLT64X_MANT_DIG__ != 113 || __FLT64X_DIG__ != 33 || __FLT64X_DECIMAL_DIG__ != 36
#error "binary128 _Float64x"
#endif
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(sizeof(_Float64x) == 16 && alignof(_Float64x) == 16);
static_assert(__FLT64X_EPSILON__ == 0x1p-112f64x && __FLT64X_MIN__ == 0x1p-16382f64x);
static_assert(__FLT64X_DENORM_MIN__ == 0x1p-16494f64x && __FLT64X_MAX__ == __FLT128_MAX__);
_Float64x y; _Float128 f128; long double ld;
static_assert(same<decltype(y + f128), _Float128> && same<decltype(f128 * y), _Float128>);
static_assert(same<decltype(y + ld), _Float64x>);
void convert() {
    _Float64x c = f128;
    _Float128 d = y;
    (void)c; (void)d;
}
