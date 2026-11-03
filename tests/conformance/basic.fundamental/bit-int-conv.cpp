// [conv.prom], [conv.rank], [expr.arith.conv] (C23 6.3.1.1, 6.3.1.8 as
// clang applies them in C++): a `_BitInt(N)` is not promoted to `int`; it
// ranks below a standard integer type of the same width and by width
// otherwise; the usual arithmetic conversions pick between them by rank and
// signedness; the type traits treat it as an integral type ([meta.unary.cat]).
//@ expect: compile-ok
template <class T, class U> constexpr bool same = __is_same(T, U);
_BitInt(8) a8; unsigned _BitInt(8) u8; _BitInt(64) a64; unsigned _BitInt(64) u64;
_BitInt(128) a128; _BitInt(33) a33; _BitInt(32) a32; unsigned _BitInt(32) u32;
short sh; char ch; bool bb; long lg; unsigned long ul;
__int128 i128; unsigned __int128 ui128;
static_assert(same<decltype(a8 + 1), int>);
static_assert(same<decltype(a8 + a8), _BitInt(8)>);
static_assert(same<decltype(+a8), _BitInt(8)>);
static_assert(same<decltype(-u8), unsigned _BitInt(8)>);
static_assert(same<decltype(~a8), _BitInt(8)>);
static_assert(same<decltype(a8 + u8), unsigned _BitInt(8)>);
static_assert(same<decltype(a8 + sh), int>);
static_assert(same<decltype(a8 + ch), int>);
static_assert(same<decltype(a8 + bb), int>);
static_assert(same<decltype(a32 + 1), int>);
static_assert(same<decltype(u32 + 1), unsigned int>);
static_assert(same<decltype(a33 + 1), _BitInt(33)>);
static_assert(same<decltype(a33 + u32), _BitInt(33)>);
static_assert(same<decltype(a33 + lg), long>);
static_assert(same<decltype(a64 + lg), long>);
static_assert(same<decltype(a64 + ul), unsigned long>);
static_assert(same<decltype(u64 + lg), unsigned long>);
static_assert(same<decltype(a128 + i128), __int128>);
static_assert(same<decltype(a128 + ui128), unsigned __int128>);
static_assert(same<decltype(a8 << 1), _BitInt(8)>);
static_assert(same<decltype(1 << a8), int>);
static_assert(same<decltype(a8 + 1.0), double>);
static_assert(same<decltype(a8 < a8), bool>);
static_assert(same<decltype(bb ? 1 : a8), int>);

// Overload resolution: an integral conversion, never a promotion.
int f(int);
long f(long);
_BitInt(8) f(_BitInt(8));
static_assert(same<decltype(f(a8)), _BitInt(8)>);
char g(int);
double g(double);
template <class T> concept callg = requires(T t) { g(t); };
static_assert(callg<short> && !callg<_BitInt(8)>);

// Traits and transformations.
static_assert(__is_integral(_BitInt(9)) && __is_arithmetic(unsigned _BitInt(1)));
static_assert(__is_signed(_BitInt(9)) && !__is_unsigned(_BitInt(9)));
static_assert(__is_unsigned(unsigned _BitInt(9)) && !__is_signed(unsigned _BitInt(9)));
static_assert(__is_scalar(_BitInt(128)) && __is_trivially_copyable(_BitInt(7)));
static_assert(same<__make_unsigned(_BitInt(9)), unsigned _BitInt(9)>);
static_assert(same<__make_signed(unsigned _BitInt(9)), _BitInt(9)>);
static_assert(same<__make_unsigned(const _BitInt(9)), const unsigned _BitInt(9)>);
static_assert(!same<_BitInt(32), int> && !same<_BitInt(8), signed char>);
static_assert(same<_BitInt(8), signed _BitInt(8)>);
static_assert(same<_BitInt(8) unsigned, unsigned _BitInt(8)>);

// `bool` conversion and functional casts.
constexpr bool t = _BitInt(9)(3);
static_assert(t && !(unsigned _BitInt(1))0);
static_assert(sizeof(_BitInt(8)) == 1 && sizeof(unsigned _BitInt(16)) == 2);
static_assert(alignof(_BitInt(32)) == alignof(int));
