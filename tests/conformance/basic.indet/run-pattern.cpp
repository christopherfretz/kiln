//@ expect: run
//@ args: -ftrivial-auto-var-init=pattern -Wno-vla-cxx-extension
//@ stdout: int aaaaaaaa
//@ stdout: ptr aaaaaaaaaaaaaaaa
//@ stdout: float ffffffff
//@ stdout: padded aa aa aa aa aa aa aa aa aa aa aa aa
//@ stdout: ctor 1 aaaaaaaa
//@ stdout: vla aaaaaaaa aaaaaaaa aaaaaaaa
// -ftrivial-auto-var-init=pattern (spec/driver.md, spec/codegen.md §7y):
// clang's initialization pattern, here read back at run time. Integers
// and padding are 0xAA bytes, pointers 0xAAAA..., floating point all
// ones; a class with a non-trivial default constructor and a VLA are
// filled too. Both compilers run it (the flag is clang's).
#include <cstring>
extern "C" int printf(const char *, ...);
struct Padded {
  char c;
  int i;
  short s;
};
struct Ctor {
  Ctor() : set(1) {}
  int set;
  unsigned unset;
};
[[gnu::noinline]] unsigned read_int() {
  unsigned x;
  unsigned r;
  std::memcpy(&r, &x, sizeof r);
  return r;
}
[[gnu::noinline]] unsigned long read_ptr() {
  int *p;
  unsigned long r;
  std::memcpy(&r, &p, sizeof r);
  return r;
}
[[gnu::noinline]] unsigned read_float() {
  float f;
  unsigned r;
  std::memcpy(&r, &f, sizeof r);
  return r;
}
[[gnu::noinline]] void read_padded() {
  Padded p;
  unsigned char b[sizeof p];
  std::memcpy(b, &p, sizeof p);
  printf("padded");
  for (unsigned char c : b)
    printf(" %x", c);
  printf("\n");
}
[[gnu::noinline]] void read_ctor() {
  Ctor c;
  unsigned r;
  std::memcpy(&r, &c.unset, sizeof r);
  printf("ctor %d %x\n", c.set, r);
}
[[gnu::noinline]] void read_vla(int n) {
  unsigned a[n];
  printf("vla");
  for (int k = 0; k < n; ++k) {
    unsigned r;
    std::memcpy(&r, &a[k], sizeof r);
    printf(" %x", r);
  }
  printf("\n");
}
int main() {
  printf("int %x\n", read_int());
  printf("ptr %lx\n", read_ptr());
  printf("float %x\n", read_float());
  read_padded();
  read_ctor();
  read_vla(3);
}
