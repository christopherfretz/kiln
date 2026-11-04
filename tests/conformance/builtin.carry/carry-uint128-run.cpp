// A two-limb 128-bit unsigned integer whose operator+= chains
// __builtin_addcll over its limbs, guarded by __has_builtin as portable
// formatting libraries do (clang LanguageExtensions "Multiprecision
// Arithmetic Builtins"), at run time and in a constant expression.
// Verified against clang 19.
//@ expect: run
//@ stdout: 0 1 1
//@ stdout: 0000000000000001 ffffffffffffffff
//@ stdout: 0000000000000002 0000000000000000
//@ stdout: 123456789abcdef1 11111111111110f0
extern "C" int printf(const char *, ...);
struct u128 {
  unsigned long long lo, hi;
  constexpr u128 &operator+=(unsigned long long n) {
#if __has_builtin(__builtin_addcll)
    unsigned long long carry = 0;
    lo = __builtin_addcll(lo, n, 0, &carry);
    hi += carry;
#else
    unsigned long long s = lo + n;
    hi += s < lo;
    lo = s;
#endif
    return *this;
  }
  constexpr u128 &operator+=(u128 n) {
    unsigned long long c = 0;
    lo = __builtin_addcll(lo, n.lo, 0, &c);
    hi = __builtin_addcll(hi, n.hi, c, &c);
    return *this;
  }
};
constexpr u128 sum(u128 a, unsigned long long n) { return a += n; }
static_assert(sum({~0ull, 0}, 1).hi == 1 && sum({~0ull, 0}, 1).lo == 0);
static_assert(sum({5, 7}, 6).hi == 7 && sum({5, 7}, 6).lo == 11);
template <class T> T opaque(T v) {
  volatile T x = v;
  return x;
}
void show(u128 v) { printf("%016llx %016llx\n", v.hi, v.lo); }
int main() {
  u128 a{opaque(~0ull), 0};
  a += opaque(1ull);
  printf("%llu %llu %d\n", a.lo, a.hi, sum({~0ull, 0}, 1).hi == 1);
  show(u128{opaque(~0ull), 1});
  u128 c{~0ull, 1};
  c += opaque(1ull);
  show(c);
  u128 d{0x0123456789abcdefull << 4, 0x123456789abcdef0ull - 1};
  d += u128{opaque(0xfedcba9876543200ull), 1};
  show(d);
}
