// [over.ics.rank]/4.3, [conv.fpprom], [conv.double] (C++23, P1467R9): a
// floating-point conversion between types of equal conversion rank is better
// than one to a non-floating type or to a floating type of different rank,
// and between equal-rank targets the greater subrank wins. Extended types do
// not promote, so float -> double stays the only floating promotion. Every
// choice agrees with GCC 14.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ xfail(clang): neither clang 19 nor clang 22 implements P1467 (no _Float32/_Float64)
template<class T, class U> constexpr bool same = false;
template<class T> constexpr bool same<T, T> = true;
_Float16 h; __bf16 b; _Float32 f32; _Float64 f64; _Float128 f128;
float f; double d;

// float -> _Float32 (equal rank) beats float -> _Float64.
char a(_Float32); int a(_Float64);
static_assert(same<decltype(a(f)), char>);
// float -> _Float32 (equal rank) beats float -> int.
char c(_Float32); int c(int);
static_assert(same<decltype(c(f)), char>);
// _Float32 -> float (equal rank) beats _Float32 -> double; no promotion.
char e(float); int e(double);
static_assert(same<decltype(e(f32)), char>);
// double -> _Float64 (equal rank) beats double -> long double.
char g(_Float64); int g(long double);
static_assert(same<decltype(g(d)), char>);
// float -> double is a promotion, which beats any conversion.
char k(_Float32); int k(double);
static_assert(same<decltype(k(f)), int>);
// _Float32 -> double is a conversion, not a promotion: equal rank wins.
char m(float); int m(double); long m(_Float64);
static_assert(same<decltype(m(f32)), char>);
// Types of equal rank stay distinct: each argument matches its own exactly.
char p(float); int p(_Float32);
static_assert(same<decltype(p(_Float32{})), int> && same<decltype(p(float{})), char>);
// float -> _Float64 vs float -> _Float128: neither has equal rank, so both
// are plain conversions and the call is ambiguous; double breaks the tie by
// promotion.
char s(_Float64); int s(_Float128); long s(double);
static_assert(same<decltype(s(f)), long>);
// _Float16 and __bf16 are not promoted: both conversions to float and
// double rank the same, so the exact match decides.
char q(float); int q(__bf16);
static_assert(same<decltype(q(b)), int> && same<decltype(q(h)), char>);
// _Float128 -> long double vs _Float128 -> double: neither has equal rank
// on every target, so call with _Float64 instead: equal rank to double.
char r(double); int r(_Float128);
static_assert(same<decltype(r(f64)), char>);
