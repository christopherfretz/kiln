// [basic.extended.fp], [conv.rank]/2, [conv.double]/2: on x86-64 GCC's
// _Float64x is the x87 80-bit format, a strict subset of _Float128's
// binary128, so _Float128 -> _Float64x converts to a type of lesser rank.
// The __FLT64X_*__ values are x86_64-linux-gnu-g++-14 -dM -E's.
// Reference: GCC 14 (WP-380).
//@ expect: compile-ok
//@ std: c++23
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 28:19 warning "ISO C++ does not allow converting to '_Float64x' from '_Float128' with greater conversion rank"
//@ skip(clang): clang 19 and 22 have no _Float64x (GCC is the reference)
#if __FLT64X_MANT_DIG__ != 64 || __FLT64X_DIG__ != 18 || __FLT64X_DECIMAL_DIG__ != 21
#error "x87 _Float64x"
#endif
#if __FLT64X_MIN_EXP__ != -16381 || __FLT64X_MAX_EXP__ != 16384 || __FLT64X_MIN_10_EXP__ != -4931
#error "x87 _Float64x exponents"
#endif
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(sizeof(_Float64x) == 16 && alignof(_Float64x) == 16);
static_assert(__FLT64X_EPSILON__ == 0x1p-63f64x && __FLT64X_MIN__ == 0x1p-16382f64x);
static_assert(__FLT64X_DENORM_MIN__ == 0x1p-16445f64x && __FLT64X_MAX__ == __LDBL_MAX__);
static_assert(__FLT64X_NORM_MAX__ == __FLT64X_MAX__ && __FLT64X_HAS_DENORM__ == 1);
_Float64x y; _Float128 f128;
static_assert(same<decltype(y + f128), _Float128>);
static_assert(1.0f64x + 0x1p-63f64x != 1.0f64x && 1.0f64x + 0x1p-64f64x == 1.0f64x);
void narrow() {
    _Float64x c = f128;
    (void)c;
}
