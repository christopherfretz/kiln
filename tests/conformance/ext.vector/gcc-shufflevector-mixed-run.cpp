// GCC's __builtin_shufflevector(v0, v1, i...) with vectors of the same
// element type and different lengths (clang requires the same type): the
// indices count over v0's lanes then v1's (index Na + Nb - 1 is v1's last
// lane), -1 is a don't-care lane, and the result has one lane per index.
// Run time, -O2-style folding of constant operands, a constant
// expression, and a one-lane vector widened against an eight-lane one.
// Found running GCC's C++ testsuite. Compared with GCC 16.1. WP-1548.
//@ std: c++20
//@ expect: run
//@ requires: clang>=22
//@ oracle: gcc
//@ skip(clang): clang 19/22 require both operands of __builtin_shufflevector to have the same type
//@ strict-diags
//@ args: -Wno-psabi
//@ stdout: 0 8 1 9 | 0 4 1 5 | 0 4 1 5 6 7 2 3 | 15 0 | 5 | 7
extern "C" int printf(const char *, ...);
typedef int v4si __attribute__((vector_size(16)));
typedef int v8si __attribute__((vector_size(32)));
typedef int v2si __attribute__((vector_size(8)));
typedef unsigned char v1qi __attribute__((vector_size(1)));
typedef unsigned char v8qi __attribute__((vector_size(8)));

__attribute__((noinline)) v4si lo_hi(v4si a, v8si b) { return __builtin_shufflevector(a, b, 0, 4, 1, 5); }
__attribute__((noinline)) v4si hi_lo(v8si a, v4si b) { return __builtin_shufflevector(a, b, 0, 8, 1, 9); }
__attribute__((noinline)) v8si wide(v8si a, v4si b) {
  return __builtin_shufflevector(a, b, 0, 8, 1, 9, 10, 11, 2, 3);
}
__attribute__((noinline)) v2si last(v4si a, v8si b) { return __builtin_shufflevector(a, b, 11, -1); }

constexpr v2si c2 = {5, 6};
constexpr v8si c8 = {0, 1, 2, 3, 4, 5, 6, 7};
static_assert(__builtin_shufflevector(c2, c8, 9, 0, 1, 2)[0] == 7);
static_assert(__builtin_shufflevector(c8, c2, 8, 9, 7, 0)[1] == 6);

int main() {
  v4si a = {0, 1, 2, 3};
  v8si a8 = {0, 1, 2, 3, 4, 5, 6, 7}, b8 = {8, 9, 10, 11, 12, 13, 14, 15};
  v4si b = {4, 5, 6, 7};
  v4si r = lo_hi(a, b8);
  v4si s = hi_lo(a8, b);
  v8si w = wide(a8, b);
  v2si l = last(a, b8);
  v1qi u = __builtin_shufflevector((v1qi){5}, (v8qi){1, 2, 3, 4, 5, 6, 7, 8}, 0);
  v1qi t = __builtin_shufflevector((v1qi){}, (v8qi){1, 2, 3, 4, 5, 6, 7, 8}, 7);
  printf("%d %d %d %d | %d %d %d %d | %d %d %d %d %d %d %d %d | %d %d | %d | %d\n", r[0], r[1],
         r[2], r[3], s[0], s[1], s[2], s[3], w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7], l[0],
         c2[0] - 5, u[0], t[0]);
}
