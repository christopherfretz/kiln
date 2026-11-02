// __builtin_alloca and __builtin_alloca_with_align: the alignment is a
// constant power of two in bits, at least 8 and at most INT_MAX (clang
// SemaBuiltinAllocaWithAlign); neither is a constant expression
// ([expr.const]/10) nor can it be named without a call. Verified against
// clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:3 error "argument to '__builtin_alloca_with_align' must be a constant integer"
//@ diag: 23:3 error "requested alignment is not a power of 2"
//@ diag: 24:3 error "requested alignment must be 8 or greater"
//@ diag: 26:3 error "requested alignment must be 2147483647 or smaller"
//@ diag: 28:20 warning "implicit conversion from 'double' to 'unsigned long' changes value from 1.5 to 1"
//@ diag: 29:20 error "too few arguments to function call, expected 1, have 0"
//@ diag: 30:12 error "builtin functions must be directly called"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:32 note "subexpression not valid in a constant expression"
//@ diag: 33:15 note "in call to 'g2()'"
//@ diag: 35:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:32 note "subexpression not valid in a constant expression"
//@ diag: 35:15 note "in call to 'g3()'"
void f(unsigned long n) {
  __builtin_alloca_with_align(4, n);
  __builtin_alloca_with_align(4, 3);
  __builtin_alloca_with_align(4, 4);
  __builtin_alloca_with_align(4, 16);
  __builtin_alloca_with_align(4, 1ull << 33);
  __builtin_alloca_with_align(4, 8);
  __builtin_alloca(1.5);
  __builtin_alloca();
  auto p = __builtin_alloca;
}
constexpr int g2() { void *p = __builtin_alloca(4); return p != nullptr; }
static_assert(g2());
constexpr int g3() { void *p = __builtin_alloca_with_align(4, 64); return p != nullptr; }
static_assert(g3());
