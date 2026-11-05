// [lex.fcon] (C++17, P0245R1): hexadecimal floating-point literals.
// WP-646 feature probe.
//@ expect: run
//@ std: c++17
//@ strict-diags
//@ stdout: 1 0.5 10 255.5 3
extern "C" int printf(const char*, ...);
static_assert(0x1p0 == 1.0 && 0x1p-1 == 0.5 && 0x1.4p3 == 10.0);
static_assert(0xA.Fp0f == 10.9375f && 0x1P+4L == 16.0L);
static_assert(0x.8p1 == 1.0 && sizeof(0x1p0f) == sizeof(float));
int main() {
  printf("%g %g %g %g %d\n", 0x1p0, 0x.8p0, 0x1.4p3, 0xff.8p0,
         (int)0x1.8p1);
}
