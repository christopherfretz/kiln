// [expr.const]: the alignment builtins on integers are constant
// expressions (clang ExprConstant BI__builtin_align_up / _down /
// __builtin_is_aligned): the result wraps to the source type; an
// alignment that is not a positive power of 2, or exceeds 2^(N-1) for an
// N-bit source, makes the call non-constant with a note at the
// alignment. Same at clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:56 note "requested alignment 3 is not a positive power of two"
//@ diag: 37:15 note "in call to 'up(3)'"
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:56 note "requested alignment 0 is not a positive power of two"
//@ diag: 38:15 note "in call to 'up(0)'"
//@ diag: 39:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:56 note "requested alignment -4 is not a positive power of two"
//@ diag: 39:15 note "in call to 'up(-4)'"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:78 note "requested alignment must be 128 or less for type 'signed char'; 256 is invalid"
//@ diag: 41:15 note "in call to 'upc(256)'"
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:66 note "requested alignment must be 2147483648 or less for type 'int'; 1099511627776 is invalid"
//@ diag: 45:15 note "in call to 'al(1099511627776)'"
static_assert(__builtin_align_up(13, 8) == 16);
static_assert(__builtin_align_down(13u, 8) == 8);
static_assert(__builtin_is_aligned(16, 8));
static_assert(!__builtin_is_aligned(12, 8));
static_assert(__builtin_align_up(-13, 8) == -8);
static_assert(__builtin_align_down(-13, 8) == -16);
static_assert(__builtin_is_aligned(-16, 16));
static_assert(__builtin_align_up(2147483647, 2) == -2147483647 - 1);
static_assert(__builtin_align_up(~0ull, 16) == 0);
static_assert(__builtin_align_up(5ull, 1ull << 63) == 1ull << 63);
static_assert(__builtin_align_up((signed char)127, 8) == -128);
constexpr int up(int a) { return __builtin_align_up(5, a); }
static_assert(up(4) == 8);
static_assert(up(3) == 8);
static_assert(up(0) == 8);
static_assert(up(-4) == 8);
constexpr signed char upc(int a) { return __builtin_align_up((signed char)5, a); }
static_assert(upc(256) == 8);
static_assert(upc(128) == -128);
static_assert(upc(64) == 64);
constexpr bool al(long long a) { return __builtin_is_aligned(16, a); }
static_assert(al(1LL << 40));
static_assert(!al(1LL << 31));
static_assert(al(16) && !al(32));
