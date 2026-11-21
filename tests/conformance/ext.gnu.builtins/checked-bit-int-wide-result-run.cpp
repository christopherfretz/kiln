// Run-time code for a _BitInt wider than 128 bits: a constant at or above
// 2^127 of an unsigned wide type is zero-extended, not sign-extended, and
// the checked arithmetic builtins store into a wide result and compute the
// bounds of a wide result type (kiln emitted invalid IR for both and
// sign-extended the constant; clang 19 emits the same invalid IR).
//@ expect: run
//@ requires: target-x86_64
//@ requires: clang>=22
//@ stdout: 1 0 1
//@ stdout: 0 1fffffffffffffffe 1
//@ stdout: 1 1 0
//@ stdout: 1 1 1
#include <cstdio>

using UW = unsigned _BitInt(192);
using SW = _BitInt(192);
using BW = unsigned _BitInt(1024);

UW g = (UW)1 << 127;

template <class T> unsigned bit(T v, int i) { return (unsigned)((v >> i) & 1); }

int main(int argc, char **) {
  std::printf("%u %u %u\n", bit(g, 127), bit(g, 128), (unsigned)(g == ((UW)1 << 127)));

  unsigned long long a = ~0ULL - (argc - 1), b = a;
  SW r = 0;
  bool o = __builtin_add_overflow(a, b, &r);
  std::printf("%d %llx%016llx %d\n", (int)o, (unsigned long long)(r >> 64),
              (unsigned long long)r, (int)(r > 0));

  int m = -argc;
  BW big = ~(BW)0;
  UW s = 0;
  bool o1 = __builtin_mul_overflow(m, big, &s);
  UW u = 0;
  bool o2 = __builtin_mul_overflow(big, (BW)argc, &u);
  std::printf("%d %d %d\n", (int)o1, (int)o2, (int)(u == ~(UW)0) ^ 1);

  BW one = (BW)argc;
  UW s2 = 0;
  bool o3 = __builtin_mul_overflow(-argc, one, &s2);
  UW u2 = 0;
  bool o4 = __builtin_sub_overflow((BW)0, one, &u2);
  SW s3 = 0;
  bool o5 = __builtin_add_overflow(((BW)argc << 191), (BW)0, &s3);
  std::printf("%d %d %d\n", (int)o3, (int)o4, (int)o5);
}
