// Variadic calls: default argument promotions, and on x86_64 the %al
// SSE-register count is implied by the call (the IR is the same).
extern "C" int printf(const char*, ...);
int show(int i, long l, double d, float f, char c, short s, unsigned u, const char* str, void* p) {
  printf("%d %ld %f %f %c %d %u %s %p\n", i, l, d, f, c, s, u, str, p);
  printf("no args\n");
  printf("%Lf\n", (long double)d);
  return printf("%lld %llu\n", 1LL, 2ULL);
}
