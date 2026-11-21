// [expr.const]: a _BitInt alignment wider than 128 bits of the alignment
// builtins is checked at its full width (clang getBuiltinAlignArguments):
// a power of two up to 2^(width-1) of the source rounds a wide source or
// a narrow one, a negative or non-power-of-two one or a larger one is not
// a constant expression (kiln saturated the alignment to 128 bits and
// rejected every alignment of 2^127 or more, and accepted negative ones).
// x86-64 only (aarch64's __BITINT_MAXWIDTH__ is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:13 error "constexpr variable 'e1' must be initialized by a constant expression"
//@ diag: 30:58 note "requested alignment must be 803469022129495137770981046170581301261101496891396417650688 or less for type 'U' (aka 'unsigned _BitInt(200)'); 1606938044258990275541962092341162602522202993782792835301376 is invalid"
//@ diag: 39:18 note "in call to"
//@ diag: 40:13 error "constexpr variable 'e2' must be initialized by a constant expression"
//@ diag: 30:58 note "requested alignment 4181389724724491839037947176121567782371328 is not a positive power of two"
//@ diag: 40:18 note "in call to"
//@ diag: 41:15 error "constexpr variable 'e3' must be initialized by a constant expression"
//@ diag: 31:62 note "requested alignment must be 2147483648 or less for type 'int'; 1393796574908163946345982392040522594123776 is invalid"
//@ diag: 41:20 note "in call to"
//@ diag: 42:16 error "constexpr variable 'e4' must be initialized by a constant expression"
//@ diag: 32:63 note "requested alignment -1393796574908163946345982392040522594123776 is not a positive power of two"
//@ diag: 42:21 note "in call to"
//@ diag: 43:16 error "constexpr variable 'e5' must be initialized by a constant expression"
//@ diag: 32:63 note "requested alignment -803469022129495137770981046170581301261101496891396417650688 is not a positive power of two"
//@ diag: 43:21 note "in call to"
using U = unsigned _BitInt(200);
using V = unsigned _BitInt(256);
using S = _BitInt(200);
constexpr U up(U x, U a) { return __builtin_align_up(x, a); }
constexpr U upv(U x, V a) { return __builtin_align_up(x, a); }
constexpr int upi(int x, U a) { return __builtin_align_up(x, a); }
constexpr bool isa(U x, S a) { return __builtin_is_aligned(x, a); }
static_assert(up(5, U(1) << 199) == U(1) << 199);
static_assert(up(5, U(1) << 130) == U(1) << 130);
static_assert(upv(5, V(1) << 150) == U(1) << 150);
static_assert(upi(5, U(16)) == 16);
static_assert(isa(U(1) << 140, S(1) << 140));
static_assert(!isa(U(1) << 140, S(1) << 141));
constexpr U e1 = upv(5, V(1) << 200);
constexpr U e2 = upv(5, V(3) << 140);
constexpr int e3 = upi(5, U(1) << 140);
constexpr bool e4 = isa(5, S(-1) << 140);
constexpr bool e5 = isa(5, S(-1) << 199);
