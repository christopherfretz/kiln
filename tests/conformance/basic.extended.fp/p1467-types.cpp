// [basic.extended.fp], [conv.rank], [expr.arith.conv] (C++23, P1467R9):
// _Float16, _Float32, _Float64, _Float128 and __bf16 (std::bfloat16_t) are
// distinct extended floating-point types, spelled by the f16/f32/f64/f128/bf16
// literal suffixes. Mixed arithmetic converts to the operand of greater rank,
// and between equal ranks to the one of greater subrank (the extended type);
// extended types are never promoted ([conv.fpprom]). Self-checking; the
// values agree with GCC 14.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ xfail(clang): neither clang 19 nor clang 22 implements P1467 (_Float32, _Float64, f32/f64/f128/bf16 suffixes)
template<class T, class U> constexpr bool same = false;
template<class T> constexpr bool same<T, T> = true;

// Distinct types ([basic.extended.fp]/1).
static_assert(!same<_Float32, float> && !same<_Float64, double>);
static_assert(!same<_Float128, long double>);
#ifdef __x86_64__
static_assert(!same<_Float128, __float128>); // __float128 is x86-64 only
#endif
static_assert(!same<__bf16, _Float16> && !same<_Float32, _Float64>);
static_assert(sizeof(_Float16) == 2 && sizeof(__bf16) == 2);
static_assert(sizeof(_Float32) == 4 && sizeof(_Float64) == 8 && sizeof(_Float128) == 16);
static_assert(alignof(_Float32) == 4 && alignof(_Float64) == 8 && alignof(_Float128) == 16);

// [lex.fcon]: the literal suffixes name the types.
static_assert(same<decltype(1.0f16), _Float16> && same<decltype(1.0F16), _Float16>);
static_assert(same<decltype(1.0f32), _Float32> && same<decltype(1.0F32), _Float32>);
static_assert(same<decltype(1.0f64), _Float64> && same<decltype(1.0F64), _Float64>);
static_assert(same<decltype(1.0f128), _Float128> && same<decltype(1.0F128), _Float128>);
static_assert(same<decltype(1.0bf16), __bf16> && same<decltype(1.0BF16), __bf16>);
static_assert(same<decltype(0x1p-3f32), _Float32> && same<decltype(1e5bf16), __bf16>);

// [expr.arith.conv]/1.4: greater rank wins; at equal rank, greater subrank.
_Float16 h; __bf16 b; _Float32 f32; _Float64 f64; _Float128 f128;
float f; double d; long double ld;
static_assert(same<decltype(f32 + f), _Float32> && same<decltype(f + f32), _Float32>);
static_assert(same<decltype(f64 + d), _Float64> && same<decltype(d * f64), _Float64>);
static_assert(same<decltype(f32 + f64), _Float64> && same<decltype(f32 + d), double>);
static_assert(same<decltype(f64 + f), _Float64> && same<decltype(h + f32), _Float32>);
static_assert(same<decltype(b + f), float> && same<decltype(b + f32), _Float32>);
static_assert(same<decltype(h + f), float> && same<decltype(b + d), double>);
static_assert(same<decltype(f128 + f64), _Float128> && same<decltype(f128 + 1), _Float128>);
static_assert(same<decltype(f128 + ld), _Float128>);
static_assert(same<decltype(b + 1), __bf16> && same<decltype(h * 2u), _Float16>);
static_assert(same<decltype(b + b), __bf16> && same<decltype(h - h), _Float16>);

// [conv.fpprom]: no promotion of extended types; unary + keeps the type.
static_assert(same<decltype(+b), __bf16> && same<decltype(-h), _Float16>);
static_assert(same<decltype(+f32), _Float32> && same<decltype(+f128), _Float128>);

// [expr.cond]/7.3: the usual arithmetic conversions in ?:.
bool c;
static_assert(same<decltype(c ? f32 : f), _Float32> && same<decltype(c ? f64 : f32), _Float64>);
static_assert(same<decltype(c ? b : f), float> && same<decltype(c ? b : 1.0bf16), __bf16>);
