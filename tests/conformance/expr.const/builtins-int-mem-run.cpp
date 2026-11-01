// [bit.rotate], [cstring.syn], [cwchar.syn]: the GCC integer builtins and
// `bcmp`/`wmemcpy`/`wmemmove`/`memset`/`mempcpy` at run time (operands read
// from volatiles, so nothing folds) agree with clang.
//@ expect: run
//@ stdout: 0 4 1 41
//@ stdout: 1 0 31 62 1
//@ stdout: 128 36608 2147483648 576460752303423488
//@ stdout: 3 2 128 1
//@ stdout: 0 1 abc aab
//@ stdout: xxc 2 ab
extern "C" int printf(const char *, ...);
typedef decltype(sizeof 0) size_t;
volatile int vz = 0, v8 = 8, vm1 = -1;
volatile long long v40 = 1LL << 40, vneg = -1LL << 62;
volatile unsigned v7 = 7, vone = 1;
volatile unsigned long vl3 = 3, vl16 = 0x10;
volatile unsigned char vc81 = 0x81, vc1 = 1, vamt = 9;
volatile unsigned short vs = 0x00f1;
volatile size_t n2 = 2, n3 = 3;
int main() {
  printf("%d %d %d %d\n", __builtin_ffs(vz), __builtin_ffs(v8), __builtin_ffs(vm1),
         __builtin_ffsll(v40));
  printf("%d %d %d %d %d\n", __builtin_parity(v7), __builtin_parityl(vl3), __builtin_clrsb(vm1),
         __builtin_clrsbl(vone), __builtin_clrsbll(vneg));
  printf("%d %d %u %lu\n", __builtin_bitreverse8(vc1), __builtin_bitreverse16(vs),
         __builtin_bitreverse32(vone), __builtin_bitreverse64(vl16));
  printf("%d %d %d %u\n", __builtin_rotateleft8(vc81, vc1), __builtin_rotateleft16(vc1, 17),
         __builtin_rotateright8(vc1, vamt), __builtin_rotateright32(0x10u, 4 + vz));
  char a[4] = "abc", b[4] = "abd";
  wchar_t wa[4] = L"abc", wb[4] = {};
  __builtin_wmemcpy(wb, wa, n3);
  __builtin_wmemmove(wb + 1, wb, n2);
  printf("%d %d %ls %ls\n", __builtin_bcmp(a, b, n2), __builtin_bcmp(a, b, n3) != 0, wa, wb);
  char c[4] = "abc", d[4] = {};
  void *p = __builtin_memset(c, 'x' + vz, n2);
  char *e = (char *)__builtin_mempcpy(d, a, n2);
  printf("%s %d %s\n", (char *)p, (int)(e - d), d);
}
