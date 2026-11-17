// GCC's __builtin_shuffle(v, mask) and __builtin_shuffle(v0, v1, mask)
// ("Vector Extensions"): lane i of the result is lane mask[i] taken
// modulo N (one operand) or 2N (two operands, v0's lanes then v1's), the
// mask an integer vector with the operands' lane count and lane size, not
// necessarily constant (negative lanes and lanes past the end wrap).
// Every lane type on 128-, 256- and 512-bit vectors, a variable mask
// (through a noinline function), a constant one, and dependent operands
// in templates; each result is checked against the scalar rule. Found
// running GCC's C++ testsuite. Compared with GCC 16.1 (clang has no
// __builtin_shuffle). WP-1548.
//@ std: c++20
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __builtin_shuffle
//@ strict-diags
//@ args: -Wno-psabi
//@ stdout: 8 3 6 7 | 4 4 2 2 | 4.5 3.5 | 0 failures
extern "C" int printf(const char *, ...);

template <class T, int Bytes> using vec __attribute__((vector_size(Bytes))) = T;

int failures;

template <class V, class M, int N>
__attribute__((noinline)) V one(V a, M m) {
  return __builtin_shuffle(a, m);
}
template <class V, class M, int N>
__attribute__((noinline)) V two(V a, V b, M m) {
  return __builtin_shuffle(a, b, m);
}

template <class T, class I, int Bytes> void check() {
  constexpr int N = Bytes / sizeof(T);
  using V = vec<T, Bytes>;
  using M = vec<I, Bytes>;
  V a, b;
  M m;
  for (int i = 0; i < N; ++i) {
    a[i] = T(i + 1);
    b[i] = T(100 + i);
    // Lanes past 2N and negative ones (for a signed mask) as well.
    m[i] = I((i * 7 + 3) * (i % 3 == 0 ? -1 : 1) + (i % 5 == 0 ? 4 * N : 0));
  }
  V r1 = one<V, M, N>(a, m);
  V r2 = two<V, M, N>(a, b, m);
  V c1 = __builtin_shuffle(a, (M){1});
  for (int i = 0; i < N; ++i) {
    unsigned long long k = (unsigned long long)m[i];
    if (r1[i] != a[k % N])
      ++failures;
    unsigned long long j = k % (2 * N);
    if (r2[i] != (j < (unsigned)N ? a[j] : b[j - N]))
      ++failures;
    if (c1[i] != (i == 0 ? a[1] : a[0]))
      ++failures;
  }
}

template <int Bytes> void all() {
  check<signed char, signed char, Bytes>();
  check<unsigned char, unsigned char, Bytes>();
  check<short, unsigned short, Bytes>();
  check<int, int, Bytes>();
  check<unsigned, int, Bytes>();
  check<long long, long long, Bytes>();
  check<float, int, Bytes>();
  check<double, unsigned long long, Bytes>();
}

typedef int v4si __attribute__((vector_size(16)));
typedef double v2df __attribute__((vector_size(16)));
typedef long long v2di __attribute__((vector_size(16)));

int main() {
  all<16>();
  all<32>();
  all<64>();
  v4si a = {1, 2, 3, 4}, b = {5, 6, 7, 8};
  v4si r = __builtin_shuffle(a, b, (v4si){7, 2, 5, 14});
  v4si s = __builtin_shuffle(a, (v4si){3, 3, -3, 9});
  v2df x = __builtin_shuffle((v2df){1.5, 2.5}, (v2df){3.5, 4.5}, (v2di){3, -2});
  printf("%d %d %d %d | %d %d %d %d | %g %g | %d failures\n", r[0], r[1], r[2], r[3], s[0],
         s[1], s[2], s[3], x[0], x[1], failures);
}
