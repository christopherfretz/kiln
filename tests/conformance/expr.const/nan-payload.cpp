// [expr.const], [conv.double], [basic.fundamental]/12: a floating
// conversion of a NaN in a constant expression keeps its sign and its
// payload, shifted by the difference in precision, and quiets it, as
// clang's APFloat::convert does; negation keeps a signaling NaN signaling.
//@ expect: compile-ok
//@ strict-diags
using u64 = unsigned long long;
using u32 = unsigned;
using u16 = unsigned short;

constexpr double d(u64 b) { return __builtin_bit_cast(double, b); }
constexpr u64 bd(double x) { return __builtin_bit_cast(u64, x); }
constexpr u32 bf(float x) { return __builtin_bit_cast(u32, x); }
constexpr u16 bh(_Float16 x) { return __builtin_bit_cast(u16, x); }
constexpr u16 bb(__bf16 x) { return __builtin_bit_cast(u16, x); }

constexpr double nq = d(0xfff8000000000000);   // -qNaN
constexpr double sn = d(0x7ff4000000000000);   // sNaN
constexpr double big = d(0xfff123456789abcd);  // -sNaN, wide payload
constexpr double big2 = d(0x7ff023456789abcd); // sNaN, quiet bit clear
constexpr double tiny = d(0x7ff0000000000001); // sNaN, payload below half's

// Narrowing: the payload truncates, the sign stays, the result is quiet.
static_assert(bh((_Float16)nq) == 0xfe00);
static_assert(bh((_Float16)sn) == 0x7f00);
static_assert(bh((_Float16)big) == 0xfe48);
static_assert(bh((_Float16)big2) == 0x7e08);
static_assert(bh((_Float16)tiny) == 0x7e00);
static_assert(bb((__bf16)big) == 0xffc9);
static_assert(bb((__bf16)big2) == 0x7fc1);
static_assert(bf((float)big) == 0xffc91a2b);
static_assert(bf((float)tiny) == 0x7fc00000);
static_assert(bh((_Float16)__builtin_nanf("0x7fffff")) == 0x7fff);

// Widening: the payload shifts left.
static_assert(bd((double)(_Float16)big) == 0xfff9200000000000);
static_assert(bd((double)(float)big) == 0xfff9234560000000);
static_assert(bd((double)(__bf16)big2) == 0x7ff8200000000000);
static_assert(bd((double)(long double)big) == 0xfff923456789abcd);
static_assert(bd((double)(long double)(float)big) == 0xfff9234560000000);
#if __LDBL_MANT_DIG__ == 113
static_assert(__builtin_bit_cast(unsigned __int128, (long double)big) ==
              ((unsigned __int128)0xffff923456789abc << 64 | (unsigned __int128)0xd << 60));
#endif

// Negation and unary plus are not arithmetic: no quieting.
static_assert(bd(-sn) == 0xfff4000000000000);
static_assert(bd(+sn) == 0x7ff4000000000000);
static_assert(bh(-(_Float16)nq) == 0x7e00);
