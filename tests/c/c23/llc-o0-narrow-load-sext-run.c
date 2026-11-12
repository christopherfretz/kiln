// C23 6.3.1.1 (integer promotions), 6.3.1.3, 6.2.5 (_BitInt): an
// unsigned 16- or 8-bit value promoted to int, then converted to a signed
// 64-bit type, keeps its value. Guards kiln against an LLVM 19/22
// GlobalISel miscompile on AArch64 (the default `llc -O0` selector),
// found by the hod project (github.com/christopherfretz/hod,
// tests/ir/selector_narrow_load.ll): a `zext` of a narrow load whose i32
// result is both `sext`ed to i64 and `zext`ed to an odd width becomes a
// sign-extending load. kiln's -O0 IR never gives one promoted value both
// uses (every shape below loads again, or goes through memory), so these
// pass; docs/KNOWN_ISSUES.md "LLVM code generator bugs".
//@ expect: run
//@ std: c23
//@ stdout: 60128 60128 60128 60128 120257 60128 60128 60128
//@ stdout: 60128 60128 120257 120256 224 60128 180384 120256
//@ stdout: 60129 180384 3615376384 -60128 60128 60128 120256 60128 120256
int printf(const char *, ...);
unsigned short g = 0xEAE0;
unsigned char gc = 0xE0;
struct BF {
  unsigned x : 16;
  unsigned y : 16;
  int s : 17;
} bf = {0xEAE0, 0xEAE1, 0xEAE0};
enum E : unsigned short { EV = 0xEAE0 };
enum E ge = EV;
unsigned _BitInt(16) gb = 0xEAE0;
__attribute__((noinline)) long long cast_int(void) { return (long long)(int)g; }
__attribute__((noinline)) long long cast_unsigned(void) { return (long long)(unsigned)g; }
__attribute__((noinline)) long long implicit(void) { return g; }
__attribute__((noinline)) long long bitfield(void) { return (long long)(int)bf.x; }
__attribute__((noinline)) long long bitfields(void) { return bf.x + bf.y; }
__attribute__((noinline)) long long enum_ut(void) { return (long long)(int)ge; }
__attribute__((noinline)) long long bitint16(void) { return (_BitInt(64))(int)gb; }
__attribute__((noinline)) long long both(void) {
  return (long long)(int)g | (long long)(unsigned _BitInt(33))(unsigned)g;
}
__attribute__((noinline)) long long both_locals(void) {
  unsigned _BitInt(33) c = (unsigned)g;
  long long d = (int)g;
  return d | (long long)c;
}
__attribute__((noinline)) long long compound(void) {
  long long r = 0;
  r += g;
  r |= (int)g;
  return r;
}
__attribute__((noinline)) long long compound_bitint(void) {
  _BitInt(33) b = 0;
  b += g;
  return (long long)b;
}
__attribute__((noinline)) long long compound_bitint40(void) {
  unsigned _BitInt(40) b = 1;
  b |= g;
  return (long long)b + (long long)(int)g;
}
__attribute__((noinline)) long long via_int(void) {
  int i = g;
  return (long long)i + (long long)(unsigned _BitInt(48))(unsigned)i;
}
__attribute__((noinline)) long long char_load(void) {
  return (long long)(int)gc | (long long)(unsigned _BitInt(37))(unsigned)gc;
}
__attribute__((noinline)) long long signed_bitfield(void) { return (long long)bf.s; }
__attribute__((noinline)) long long signed_bitint(void) {
  return (long long)(signed _BitInt(33))(int)g * 2 + (long long)(int)g;
}
__attribute__((noinline)) long long local_copy(void) {
  unsigned short l = g;
  return (long long)(int)l + (long long)(unsigned _BitInt(33))l;
}
__attribute__((noinline)) long long add_overflow(void) {
  _BitInt(40) r;
  __builtin_add_overflow(g, 1u, &r);
  return (long long)r;
}
__attribute__((noinline)) long long mul_overflow(void) {
  _BitInt(40) r;
  __builtin_mul_overflow((int)g, 3u, &r);
  return (long long)r;
}
__attribute__((noinline)) long long mixed_mul_overflow(void) {
  unsigned _BitInt(33) r;
  __builtin_mul_overflow((int)g, (unsigned)g, &r);
  return (long long)r;
}
__attribute__((noinline)) long long wide_mul_overflow(void) {
  _BitInt(50) r;
  __builtin_mul_overflow(g, (_BitInt(35))-1, &r);
  return (long long)r;
}
__attribute__((noinline)) long long in_switch(void) {
  switch (g) {
  case 0xEAE0:
    return (long long)(int)g;
  default:
    return 0;
  }
}
__attribute__((noinline)) long long conditional(void) {
  return g ? (long long)(int)g : (long long)(unsigned _BitInt(33))g;
}
__attribute__((noinline)) long long assign_value(void) {
  unsigned _BitInt(33) b = 0;
  return (long long)(b += g) + (long long)(int)g;
}
__attribute__((noinline)) long long compound_wide(void) {
  long long b = 0;
  b += (unsigned _BitInt(45))(unsigned)g;
  return b;
}
__attribute__((noinline)) long long abs_and_wide(void) {
  return __builtin_llabs((int)g) + (long long)(unsigned _BitInt(39))(unsigned)g;
}
int main(void) {
  printf("%lld %lld %lld %lld %lld %lld %lld %lld\n", cast_int(), cast_unsigned(), implicit(), bitfield(),
         bitfields(), enum_ut(), bitint16(), both());
  printf("%lld %lld %lld %lld %lld %lld %lld %lld\n", both_locals(), compound(), compound_bitint40(), via_int(),
         char_load(), signed_bitfield(), signed_bitint(), local_copy());
  printf("%lld %lld %lld %lld %lld %lld %lld %lld %lld\n", add_overflow(), mul_overflow(), mixed_mul_overflow(),
         wide_mul_overflow(), in_switch(), conditional(), assign_value(), compound_wide(), abs_and_wide());
  return compound_bitint() == 60128 ? 0 : 1;
}
