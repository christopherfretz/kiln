//@ expect: run
//@ requires: clang>=23
//@ oracle: none
//@ stdout: int 0
//@ stdout: int[16] 0
//@ stdout: padded 0
//@ stdout: loop 0 0 0
// [basic.indet]/1-2 (C++26, P2795R5): an automatic variable without an
// initializer has an erroneous value, and reading it is erroneous
// behaviour whose result is that value; kiln's erroneous value is all-zero
// bytes, padding included (spec/codegen.md §7r). The bytes are inspected
// through `unsigned char`, which [basic.indet]/2 exempts. Each variable
// is dirtied by an earlier frame at the same stack slot first, so a
// missing fill shows. clang 22 has no erroneous values (`oracle: none`).
extern "C" int printf(const char *, ...);
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
[[gnu::noinline]] int read_int() {
  int x;
  return sum(&x, sizeof x);
}
[[gnu::noinline]] int read_array() {
  int a[16];
  return sum(a, sizeof a);
}
[[gnu::noinline]] int read_padded() {
  Padded p;
  return sum(&p, sizeof p);
}
int main() {
  dirty();
  printf("int %d\n", read_int());
  dirty();
  printf("int[16] %d\n", read_array());
  dirty();
  printf("padded %d\n", read_padded());
  printf("loop");
  for (int k = 0; k < 3; ++k) {
    unsigned char u;
    printf(" %d", u);
    u = 7;
    (void)u;
  }
  printf("\n");
}
