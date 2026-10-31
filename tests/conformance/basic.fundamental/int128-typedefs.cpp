// [basic.fundamental] (GNU extension): __int128 and unsigned __int128 are
// extended integer types of rank above long long; __int128_t and
// __uint128_t are implicit typedefs for them (clang's Sema::Initialize),
// which a program may redeclare.
//@ expect: compile-ok
typedef __int128 __int128_t;
typedef unsigned __int128 __uint128_t;
static_assert(sizeof(__int128_t) == 16 && alignof(__int128_t) == 16);
static_assert(sizeof(__uint128_t) == 16 && alignof(__uint128_t) == 16);
static_assert(__is_same(__int128_t, __int128));
static_assert(__is_same(__uint128_t, unsigned __int128));
static_assert(__is_integral(__int128_t) && __is_integral(__uint128_t));
static_assert(__is_arithmetic(__int128_t) && __is_scalar(__uint128_t));
static_assert(__is_signed(__int128_t) && !__is_signed(__uint128_t));
static_assert(__is_unsigned(__uint128_t) && !__is_unsigned(__int128_t));
static_assert(__is_same(__make_signed(__uint128_t), __int128));
static_assert(__is_same(__make_unsigned(__int128_t), unsigned __int128));
static_assert(__is_same(__make_signed(const __int128_t), const __int128));
// Usual arithmetic conversions: rank above long long.
static_assert(__is_same(decltype((__int128_t)0 + 0ULL), __int128));
static_assert(__is_same(decltype((__uint128_t)0 + (__int128_t)0), unsigned __int128));
static_assert(__is_same(decltype(+(__int128_t)0), __int128));
static_assert(__is_same(decltype((__int128_t)0 << 1), __int128));
static_assert(__is_same(decltype(1 << (__int128_t)1), int));
// Specializations match through the typedefs.
template <class T> struct is_i128 { static constexpr bool value = false; };
template <> struct is_i128<__int128_t> { static constexpr bool value = true; };
template <> struct is_i128<__uint128_t> { static constexpr bool value = true; };
static_assert(is_i128<__int128>::value && is_i128<unsigned __int128>::value);
static_assert(!is_i128<long long>::value);
namespace n { int __int128_t; }  // other scopes may reuse the name
