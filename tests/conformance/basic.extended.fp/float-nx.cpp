// [basic.extended.fp], [conv.rank]/2, [expr.arith.conv]/1.4, [conv.double]/2:
// GCC's _Float32x and _Float64x (ISO/IEC TS 18661-3) are extended
// floating-point types distinct from every other type. _Float32x has
// double's format, _Float64x long double's (x87 extended on x86-64,
// binary128 on AArch64). GCC 14 ranks them by value set and, for equal value
// sets, above the standard types and below _FloatN: _Float32x + double is
// _Float32x, _Float32x + _Float64 is _Float64. The f32x/f64x literal suffixes
// and the __FLT32X_*__/__FLT64X_*__ limit macros follow GCC; the warnings and
// their columns match g++-14 (whose group is -Wnarrowing).
// Reference: GCC 14 (WP-380).
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ diag: 37:19 warning "ISO C++ does not allow converting to '_Float32x' from 'long double' with greater conversion rank"
//@ diag: 38:18 warning "ISO C++ does not allow converting to '_Float32' from '_Float32x' with greater conversion rank"
//@ skip(clang): clang 19 and 22 have no _Float32x or _Float64x (GCC is the reference)
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(!same<_Float32x, double> && !same<_Float32x, _Float64>);
static_assert(!same<_Float64x, long double> && !same<_Float64x, _Float128>);
static_assert(sizeof(_Float32x) == sizeof(double) && alignof(_Float32x) == alignof(double));
static_assert(sizeof(_Float64x) == sizeof(long double) && alignof(_Float64x) == alignof(long double));
_Float32x x; _Float64x y; double d; long double ld; _Float64 f64; _Float128 f128;
static_assert(same<decltype(x + d), _Float32x> && same<decltype(d * x), _Float32x>);
static_assert(same<decltype(x + f64), _Float64> && same<decltype(f64 - x), _Float64>);
static_assert(same<decltype(y + ld), _Float64x> && same<decltype(ld / y), _Float64x>);
static_assert(same<decltype(y + f128), _Float128>);
static_assert(same<decltype(x + y), _Float64x> && same<decltype(x + 1.0f), _Float32x>);
static_assert(same<decltype(true ? x : d), _Float32x>);
static_assert(same<decltype(1.5f32x), _Float32x> && same<decltype(1.5F64x), _Float64x>);
static_assert(same<decltype(__FLT32X_MAX__), _Float32x> && same<decltype(__FLT64X_MIN__), _Float64x>);
static_assert(__FLT32X_MANT_DIG__ == __DBL_MANT_DIG__ && __FLT32X_MAX__ == __DBL_MAX__);
static_assert(__FLT32X_EPSILON__ == __DBL_EPSILON__ && __FLT32X_DENORM_MIN__ == __DBL_DENORM_MIN__);
static_assert(__FLT64X_MANT_DIG__ == __LDBL_MANT_DIG__ && __FLT64X_MAX__ == __LDBL_MAX__);
static_assert(__FLT64X_MIN_EXP__ == __LDBL_MIN_EXP__ && __FLT64X_EPSILON__ == __LDBL_EPSILON__);
void conversions() {
    _Float32x a = ld;
    _Float32 b = x;
    double e = x; long double g = y; _Float64x h = ld; _Float64 i = x;
    (void)a; (void)b; (void)e; (void)g; (void)h; (void)i;
}
constexpr _Float32x third = 1.0f32x / 3;
static_assert(third == (_Float32x)(1.0 / 3));
constexpr _Float64x lthird = 1.0f64x / 3;
static_assert(lthird == (_Float64x)(1.0L / 3));
constexpr _Complex _Float32x cz = 2.0f32x;
static_assert(__real__ cz == 2 && __imag__ cz == 0);
