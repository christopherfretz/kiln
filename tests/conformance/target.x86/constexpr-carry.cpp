// [expr.const] clang 20 and later evaluate `addcarryx` / `subborrow`: the
// sum (difference) of `a`, `b` and a carry-in counted as 1 when nonzero
// is stored through the pointer and the carry-out is the value. WP-630.
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
constexpr unsigned add(unsigned char c, unsigned a, unsigned b) {
  unsigned out = 0;
  unsigned char co = __builtin_ia32_addcarryx_u32(c, a, b, &out);
  return out + co * 1000;
}
static_assert(add(1, 0xFFFFFFFFu, 1) == 1001);
static_assert(add(7, 2, 3) == 6);
constexpr unsigned long long sub(unsigned char c, unsigned long long a, unsigned long long b) {
  unsigned long long out = 0;
  unsigned char co = __builtin_ia32_subborrow_u64(c, a, b, &out);
  return co ? ~out : out;
}
static_assert(sub(0, 5, 3) == 2);
static_assert(sub(1, 0, 0) == 0);
