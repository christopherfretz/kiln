// [expr.const]/5.8, [basic.fundamental]/1 (C23 6.2.5): constant evaluation of
// `_BitInt(N)` arithmetic wraps to N bits for the unsigned form; signed
// overflow, an over-wide shift and `INT_MIN / -1` are not core constant
// expressions. Widths up to 128 bits (target-neutral).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:15 note "constexpr bit cast involving type 'unsigned _BitInt(16)' is not yet supported"
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:15 note "constexpr bit cast involving type '_BitInt(8)' is not yet supported"
//@ diag: 45:22 error "constexpr variable 'ov' must be initialized by a constant expression"
//@ diag: 45:42 note "value 64 is outside the range of representable values of type '_BitInt(7)'"
//@ diag: 46:22 error "constexpr variable 'ov2' must be initialized by a constant expression"
//@ diag: 46:42 note "shift count 7 >= width of type '_BitInt(7)' (7 bits)"
//@ diag: 47:22 error "constexpr variable 'ov3' must be initialized by a constant expression"
//@ diag: 47:44 note "value 64 is outside the range of representable values of type '_BitInt(7)'"
//@ diag: 48:23 error "constexpr variable 'ov4' must be initialized by a constant expression"
//@ diag: 48:44 note "shift count 33 >= width of type '_BitInt(33)' (33 bits)"
//@ diag: 50:22 error "constexpr variable 'ov5' must be initialized by a constant expression"
//@ diag: 49:57 note "value 64 is outside the range of representable values of type '_BitInt(7)'"
//@ diag: 50:28 note "in call to 'neg()'"
constexpr unsigned _BitInt(7) a = 127;
static_assert((unsigned _BitInt(7))(a + 1) == 0 && a + 1 == 128);
static_assert((unsigned _BitInt(7))200 == 72 && (_BitInt(7))100 == -28);
static_assert((unsigned _BitInt(33))-1 == 8589934591ull);
static_assert(((_BitInt(33))-1 >> 1) == -1);
static_assert(((unsigned _BitInt(33))1 << 32) == 4294967296ull);
static_assert((unsigned _BitInt(33))0xffffffffu * 2 == 8589934590ull);
static_assert((_BitInt(33))8589934591ll == -1);
static_assert(~(unsigned _BitInt(7))0 == 127);
static_assert(((unsigned _BitInt(7))5 & 3) == 1 && ((unsigned _BitInt(7))5 | 3) == 7);
static_assert((unsigned _BitInt(7))100 / 7 == 14 && (unsigned _BitInt(7))100 % 7 == 2);
static_assert((_BitInt(7))-50 / (_BitInt(7))7 == -7);
constexpr unsigned _BitInt(128) big = (unsigned _BitInt(128))1 << 127;
static_assert(big > 0 && (_BitInt(128))big < 0);
static_assert((int)(_BitInt(7))-3 == -3 && (double)(_BitInt(33))-5 == -5.0);
static_assert(!(bool)(unsigned _BitInt(7))0 && (_BitInt(33))3.75 == 3);
constexpr unsigned _BitInt(7) inc() { unsigned _BitInt(7) x = 126; ++x; x++; return x; }
static_assert(inc() == 0);
constexpr _BitInt(33) sub() { _BitInt(33) x = 5; x -= 10; x *= 3; return x; }
static_assert(sub() == -15);
static_assert(__builtin_bit_cast(unsigned _BitInt(16), (short)-1) == 65535);
struct S { _BitInt(8) b; };
static_assert(__builtin_bit_cast(S, (char)1).b == 1);
constexpr _BitInt(7) ov = (_BitInt(7))63 + (_BitInt(7))1;
constexpr _BitInt(7) ov2 = (_BitInt(7))1 << 7;
constexpr _BitInt(7) ov3 = (_BitInt(7))-64 / (_BitInt(7))-1;
constexpr _BitInt(33) ov4 = (_BitInt(33))1 << 33;
constexpr _BitInt(7) neg() { _BitInt(7) x = -64; return -x; }
constexpr _BitInt(7) ov5 = neg();
