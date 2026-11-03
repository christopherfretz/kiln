//@ expect: run
//@ requires: clang>=23
//@ oracle: none
//@ stdout: ctor 0
//@ stdout: ctor[3] 0
//@ stdout: padded-init 0
//@ stdout: vla 0
// [basic.indet]/1-2 (C++26, P2795R5): storage an initialization leaves
// unwritten holds the erroneous value too: the member a non-trivial
// default constructor does not set, the padding of a class initialized
// from a non-constant, and the elements of a VLA (spec/codegen.md §7y).
// Kiln fills each variable with zero bytes before its initializer runs,
// as clang's `-ftrivial-auto-var-init=zero` does. Each variable is
// dirtied by an earlier frame at the same stack slot first, so a missing
// fill shows. clang 22 has no erroneous values (`oracle: none`).
extern "C" int printf(const char *, ...);
struct Ctor {
  Ctor() : set(1) {}
  int set;
  int unset;
};
struct Padded {
  char c;
  int i;
  short s;
};
static int sum(const void *p, unsigned n) {
  const unsigned char *b = static_cast<const unsigned char *>(p);
  int s = 0;
  for (unsigned k = 0; k < n; ++k)
    s += b[k];
  return s;
}
[[gnu::noinline]] void dirty() {
  volatile unsigned char junk[512];
  for (int k = 0; k < 512; ++k)
    junk[k] = 0xA5;
}
[[gnu::noinline]] int one() { return 1; }
[[gnu::noinline]] int read_ctor() {
  Ctor c;
  return sum(&c.unset, sizeof c.unset);
}
[[gnu::noinline]] int read_ctors() {
  Ctor c[3];
  int s = 0;
  for (auto &e : c)
    s += sum(&e.unset, sizeof e.unset);
  return s;
}
[[gnu::noinline]] int read_padded_init() {
  Padded p{1, one(), 1};
  return sum(&p, sizeof p) - 3;
}
[[gnu::noinline]] int read_vla(int n) {
  int a[n];
  return sum(a, n * sizeof(int));
}
int main() {
  dirty();
  printf("ctor %d\n", read_ctor());
  dirty();
  printf("ctor[3] %d\n", read_ctors());
  dirty();
  printf("padded-init %d\n", read_padded_init());
  dirty();
  printf("vla %d\n", read_vla(16));
}
