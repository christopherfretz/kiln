// GNU checked arithmetic at run time (clang lowers through
// llvm.{s,u}{add,sub,mul}.with.overflow in the encompassing width, with
// special cases for mixed-sign and unsigned-to-signed multiplication): the
// wrapped result is stored and the flag says whether it did not fit,
// including a bool result. Verified against clang 19.
//@ expect: run
//@ stdout: 1 -2147483648 0 -12 1 4294967295 1 0 0 1 0 -6 1 1 1 1 0 1 1 -1 1 1
//@ stdout: 0 2 1 -1 1 -2147483648
extern "C" int printf(const char *, ...);
template <class T> T opaque(T v) {
  volatile T x = v;
  return x;
}
int main() {
  int i; long long ll; unsigned u; bool b; signed char c; unsigned short us;
  __int128 w;
  int o1 = __builtin_add_overflow(opaque(0x7fffffff), opaque(1), &i);
  printf("%d %d ", o1, i);
  int o2 = __builtin_sub_overflow(opaque(-5), opaque(7u), &ll);
  printf("%d %lld ", o2, ll);
  int o3 = __builtin_sub_overflow(opaque(1u), opaque(2), &u);
  printf("%d %u ", o3, u);
  int o4 = __builtin_mul_overflow(opaque(1 << 16), opaque(1u << 16), &u);
  printf("%d %u ", o4, u);
  int o5 = __builtin_add_overflow(opaque(true), opaque(0), &b);
  printf("%d %d ", o5, b);
  int o6 = __builtin_mul_overflow(opaque(-3), opaque(2u), &i);
  printf("%d %d ", o6, i);
  int o7 = __builtin_mul_overflow(opaque(-3ll), opaque(2u), &u);
  printf("%d %d ", o7, u == 0xfffffffau);
  int o8 = __builtin_mul_overflow(opaque(0x80000000u), opaque(1u), &i);
  printf("%d %d ", o8, i == -0x7fffffff - 1);
  int o9 = __builtin_add_overflow(opaque(127), opaque(0), &c);
  printf("%d %d ", o9, c == 127);
  int o10 = __builtin_sub_overflow(opaque(0), opaque(1), &us);
  printf("%d %d ", o10, us == 0xffff ? -1 : 0);
  int o11 = __builtin_mul_overflow(opaque((__int128)1 << 100), opaque((unsigned __int128)1 << 30), &w);
  printf("%d %d\n", o11, w == 0);
  int o12 = __builtin_sadd_overflow(opaque(1), opaque(1), &i);
  printf("%d %d ", o12, i);
  int o13 = __builtin_umul_overflow(opaque(0x10000u), opaque(0x10000u), &u);
  printf("%d %d ", o13, u == 0 ? -1 : 0);
  int o14 = __builtin_ssub_overflow(opaque(0), opaque(-0x7fffffff - 1), &i);
  printf("%d %d\n", o14, i);
}
