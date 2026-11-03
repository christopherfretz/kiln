// Argument checks of __builtin_return_address (clang SemaChecking
// BuiltinConstantArgRange(0, 0xFFFF), `void *(_Constant unsigned int)`): a
// constant level in [0, 65535], warned when nonzero (-Wframe-address, in
// -Wall); the call is never a constant expression. Same at clang 19 and
// 22 (WP-432).
//@ args: -Wall
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:30 error "argument to '__builtin_return_address' must be a constant integer"
//@ diag: 15:20 error "argument value 65536 is outside the valid range [0, 65535]"
//@ diag: 16:20 error "argument value 4294967295 is outside the valid range [0, 65535]"
//@ diag: 18:22 warning "calling '__builtin_return_address' with a nonzero argument is unsafe"
//@ diag: 21:21 error "constexpr variable 'p' must be initialized by a constant expression"
void *f(unsigned n) { return __builtin_return_address(n); }
void *g() { return __builtin_return_address(0x10000); }
void *h() { return __builtin_return_address(-1); }
void *ok0() { return __builtin_return_address(0); }
void *ok1() { return __builtin_return_address(1); }
template <unsigned N> void *t() { return __builtin_return_address(N); }
void *ok2() { return t<0>(); }
constexpr void *p = __builtin_return_address(0);
