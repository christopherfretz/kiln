//@ expect: run
//@ requires: clang>=23
//@ oracle: none
//@ stdout: 0 0 0
// [basic.indet]/2 (C++26, P2795R5): reading an erroneous value through an
// `unsigned char` or `std::byte` glvalue, and copying it into another
// such object, is not erroneous behaviour; the copy carries the erroneous
// value (all-zero bytes in kiln, spec/codegen.md §7q). clang 22 has no
// erroneous values (`oracle: none`).
#include <cstddef>
extern "C" int printf(const char *, ...);
[[gnu::noinline]] void dirty() {
  volatile unsigned char junk[256];
  for (int k = 0; k < 256; ++k)
    junk[k] = 0x5A;
}
[[gnu::noinline]] int uc() {
  unsigned char c;
  unsigned char d = c;
  return d;
}
[[gnu::noinline]] int byte() {
  std::byte b;
  std::byte e = b;
  return std::to_integer<int>(e);
}
[[gnu::noinline]] int via_uc() {
  long l;
  unsigned char c = *reinterpret_cast<unsigned char *>(&l);
  return c;
}
int main() {
  dirty();
  int a = uc();
  dirty();
  int b = byte();
  dirty();
  int c = via_uc();
  printf("%d %d %d\n", a, b, c);
}
