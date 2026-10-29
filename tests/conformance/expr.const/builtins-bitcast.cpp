// [bit.cast]/3, [expr.const]: __builtin_bit_cast in constant evaluation
// reinterprets the object representation: scalars, arrays, structs with
// padding-free layouts, and round trips through byte arrays. [ptr.launder]:
// __builtin_launder is the identity in constant evaluation, and
// __builtin_unreachable is fine when it is not reached.
//@ expect: compile-ok
//@ strict-diags
static_assert(__builtin_bit_cast(unsigned, 1.0f) == 0x3f800000u);
static_assert(__builtin_bit_cast(float, 0x40490fdbu) == 3.14159274f);
static_assert(__builtin_bit_cast(unsigned long long, -0.0) == 0x8000000000000000ull);
static_assert(__builtin_bit_cast(int, -1u) == -1);
static_assert(__builtin_bit_cast(signed char, (unsigned char)200) == -56);

struct Bytes { unsigned char b[4]; };
static_assert(__builtin_bit_cast(Bytes, 0x04030201u).b[0] == 1);
static_assert(__builtin_bit_cast(Bytes, 0x04030201u).b[3] == 4);
struct Pair { unsigned short lo, hi; };
static_assert(__builtin_bit_cast(Pair, 0xbeefcafeu).lo == 0xcafe);
static_assert(__builtin_bit_cast(Pair, 0xbeefcafeu).hi == 0xbeef);
constexpr Pair p12 = {1, 2};
static_assert(__builtin_bit_cast(unsigned, p12) == 0x00020001u);

constexpr unsigned long long roundtrip(double d) {
  struct B8 { unsigned char b[8]; };
  B8 bytes = __builtin_bit_cast(B8, d);
  unsigned long long r = 0;
  for (int i = 7; i >= 0; --i)
    r = (r << 8) | bytes.b[i];
  return r;
}
static_assert(roundtrip(1.0) == 0x3ff0000000000000ull);
static_assert(__builtin_bit_cast(double, roundtrip(2.5)) == 2.5);

struct Arr { int v[2]; };
constexpr Arr a12 = {{1, 2}};
static_assert(__builtin_bit_cast(unsigned long long, a12) == 0x0000000200000001ull);
static_assert(__builtin_bit_cast(Arr, 0x0000000700000005ull).v[1] == 7);

constexpr int launder_read() {
  int x = 42;
  int *p = __builtin_launder(&x);
  *p += 1;
  return x;
}
static_assert(launder_read() == 43);
constexpr const int ci = 5;
static_assert(*__builtin_launder(&ci) == 5);

constexpr int sign(int n) {
  if (n > 0) return 1;
  if (n < 0) return -1;
  if (n == 0) return 0;
  __builtin_unreachable();
}
static_assert(sign(-3) == -1 && sign(0) == 0 && sign(9) == 1);
