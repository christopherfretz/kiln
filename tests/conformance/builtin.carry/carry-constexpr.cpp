// The multiprecision carry builtins in constant expressions ([expr.const];
// clang ExprConstant BI__builtin_addcb..): the operands, then the
// carry-out pointer, are evaluated; the result is x + y + carryin
// (x - y - carryin) modulo 2^N and the carry-out, stored through the
// pointer, is 1 when either wrapping step overflowed. Every width, with a
// carry-in of 0 and 1, with and without overflow, and a carry-in above 1.
// Verified against clang 19.
//@ expect: compile-ok
typedef unsigned char uc; typedef unsigned short us; typedef unsigned u;
typedef unsigned long ul; typedef unsigned long long ull;
template <class T, class F> constexpr bool check(F f, T x, T y, T c, T want, T carry) {
  T co = 7;
  T r = f(x, y, c, &co);
  return r == want && co == carry;
}
#define ADD(F, T, x, y, c, want, carry)                                      \
  static_assert(check<T>([](T a, T b, T ci, T *o) { return F(a, b, ci, o); }, \
                         x, y, c, want, carry))
ADD(__builtin_addcb, uc, 1, 2, 0, 3, 0);
ADD(__builtin_addcb, uc, 1, 2, 1, 4, 0);
ADD(__builtin_addcb, uc, 0xff, 1, 0, 0, 1);
ADD(__builtin_addcb, uc, 0xfe, 1, 1, 0, 1);
ADD(__builtin_addcb, uc, 0xff, 0xff, 1, 0xff, 1);
ADD(__builtin_addcb, uc, 0xfd, 1, 5, 3, 1);
ADD(__builtin_subcb, uc, 5, 2, 0, 3, 0);
ADD(__builtin_subcb, uc, 5, 2, 1, 2, 0);
ADD(__builtin_subcb, uc, 2, 5, 0, 0xfd, 1);
ADD(__builtin_subcb, uc, 2, 2, 1, 0xff, 1);
ADD(__builtin_subcb, uc, 0, 0xff, 1, 0, 1);
ADD(__builtin_addcs, us, 1000, 2000, 1, 3001, 0);
ADD(__builtin_addcs, us, 0xfffe, 1, 1, 0, 1);
ADD(__builtin_subcs, us, 3000, 1000, 1, 1999, 0);
ADD(__builtin_subcs, us, 1, 1, 1, 0xffff, 1);
ADD(__builtin_addc, u, 0x80000000u, 0x7fffffffu, 0, 0xffffffffu, 0);
ADD(__builtin_addc, u, 0x80000000u, 0x7fffffffu, 1, 0, 1);
ADD(__builtin_addc, u, 0xffffffffu, 0xffffffffu, 1, 0xffffffffu, 1);
ADD(__builtin_subc, u, 10, 3, 1, 6, 0);
ADD(__builtin_subc, u, 3, 10, 0, 0xfffffff9u, 1);
ADD(__builtin_addcl, ul, 0xfffffffful, 1, 0, 0x100000000ul, 0);
ADD(__builtin_addcl, ul, ~0ul, 0, 1, 0, 1);
ADD(__builtin_subcl, ul, 1, 0, 1, 0, 0);
ADD(__builtin_subcl, ul, 0, 0, 1, ~0ul, 1);
ADD(__builtin_addcll, ull, 0x123456789abcdefull, 0x1111111111111111ull, 0,
    0x123456789abcdefull + 0x1111111111111111ull, 0);
ADD(__builtin_addcll, ull, ~0ull, ~0ull, 0, ~0ull - 1, 1);
ADD(__builtin_addcll, ull, ~0ull, ~0ull, 1, ~0ull, 1);
ADD(__builtin_addcll, ull, ~0ull - 1, 1, 1, 0, 1);
ADD(__builtin_subcll, ull, 0, 1, 0, ~0ull, 1);
ADD(__builtin_subcll, ull, 0, ~0ull, 1, 0, 1);
ADD(__builtin_subcll, ull, 100, 1, 0, 99, 0);
// A constexpr local for the carry-out, the builtin called directly.
constexpr ull limbs() {
  ull co = 0;
  ull lo = __builtin_addcll(~0ull, 2, 0, &co);
  ull hi = __builtin_addcll(5, 0, co, &co);
  return lo == 1 && hi == 6 && co == 0 ? hi : 0;
}
static_assert(limbs() == 6);
constexpr bool chain() {
  unsigned char co = 0;
  unsigned char r = __builtin_subcb(0, 1, 0, &co);
  unsigned char r2 = __builtin_subcb(0, 0, co, &co);
  return r == 0xff && r2 == 0xff && co == 1;
}
static_assert(chain());
int main() {}
