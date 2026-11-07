// __builtin_assume_aligned's alignment is an integer constant expression
// of integral or unscoped enumeration type (clang `BuiltinConstantArg`,
// `isIntegralOrUnscopedEnumerationType`): an enumerator, also one of a
// class template specialization named in a function template (LLVM's
// `LLVM_ASSUME_ALIGNED(p, (PickAlignment<T, A>::value))` in Endian.h).
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 7 42 8
#include <cstdio>
#include <cstring>
enum { Eight = 8 };
enum Four { FourV = 4 };
template <class T, unsigned long A> struct Pick {
  enum { value = A == 0 ? alignof(T) : A };
};
template <class T, unsigned long A = 1> T read(const void *m) {
  T r;
  std::memcpy(&r, __builtin_assume_aligned(m, (Pick<T, A>::value)), sizeof(T));
  return r;
}
alignas(8) static long buf[2] = {42, 8};
int main() {
  int x = 7;
  const void *p = __builtin_assume_aligned(buf, Eight);
  const void *q = __builtin_assume_aligned(buf + 1, FourV);
  std::printf("%d %ld %ld\n", read<int>(&x), read<long, 0>(p), *(const long *)q);
}
