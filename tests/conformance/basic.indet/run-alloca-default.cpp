//@ expect: run
//@ requires: clang>=23
//@ oracle: none
//@ stdout: alloca 0
//@ stdout: aligned 0 0
//@ stdout: loop 0 0 0
// kiln's C++26 default (spec/codegen.md §7r, §7y "alloca") is
// -ftrivial-auto-var-init=zero, so the memory of __builtin_alloca and
// __builtin_alloca_with_align reads back as zero bytes without a flag. Each call first has an earlier frame dirty the same stack, so a
// missing fill shows. clang 22 fills nothing without the flag
// (`oracle: none`); run-alloca-zero.cpp is the same with the flag.
extern "C" int printf(const char *, ...);
[[gnu::noinline]] void dirty() {
  volatile unsigned char junk[1024];
  for (int k = 0; k < 1024; ++k)
    junk[k] = 0x5A;
}
static int sum(const unsigned char *b, unsigned long n) {
  int s = 0;
  for (unsigned long k = 0; k < n; ++k)
    s += b[k];
  return s;
}
[[gnu::noinline]] int read_alloca(unsigned long n) {
  auto *p = static_cast<unsigned char *>(__builtin_alloca(n));
  return sum(p, n);
}
[[gnu::noinline]] int read_aligned(unsigned long n) {
  auto *p = static_cast<unsigned char *>(__builtin_alloca_with_align(n, 256));
  int r = sum(p, n);
  return r + (reinterpret_cast<unsigned long>(p) % 32 != 0);
}
int main() {
  dirty();
  printf("alloca %d\n", read_alloca(200));
  dirty();
  int a = read_aligned(64);
  dirty();
  printf("aligned %d %d\n", a, read_aligned(300));
  printf("loop");
  for (int i = 1; i <= 3; ++i) {
    dirty();
    printf(" %d", read_alloca(100 * i));
  }
  printf("\n");
}
