// [dcl.init.list]/7.4, [conv.prom]/3: converting an unscoped enumeration
// without a fixed underlying type to its promotion type is a promotion,
// never a narrowing conversion, even when the non-constant source's
// underlying type is unsigned (clang picks `unsigned int` for an enum
// with no negative enumerators). Reduced from LLVM's
// X86FixupVectorConstants.cpp (`{c ? X86::A : X86::B, ...}`).
//@ expect: run
//@ stdout: 30000 2147483647 4294967296 5
#include <cstdio>

enum { A = 0, B = 30000 };
enum E2 { X = 1, Y = 0x7fffffff };
enum E6 { Z = 0x100000000ll };
struct Entry { int op; int width; };

int pick(bool c) {
  Entry e{c ? A : B, 1};
  return e.op;
}
int widen(E2 v) { int x{v}; return x; }
long wide(E6 v) { long x{v}; return x; }
unsigned same(E2 v) { unsigned x{v}; return x; }

int main() {
  std::printf("%d %d %ld %u\n", pick(false), widen(Y), wide(Z), same(E2(5)));
}
