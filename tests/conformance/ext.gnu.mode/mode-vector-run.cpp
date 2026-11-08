//@ expect: run
//@ strict-diags
//@ args: -Wno-deprecated-attributes
//@ stdout: 16 16 16 32 8
//@ stdout: 3 -4 255 1.5
//@ stdout: 2 2 4
// GCC mode with a vector mode `V<N>M` (clang `handleModeAttr`,
// deprecated): N elements of mode M's type, a GNU vector (`vector_size`).
// On a vector type a scalar mode replaces the element type and keeps the
// vector's size; a vector mode gives N elements of the mode.
#include <cstdio>

template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };

typedef int v4si __attribute__((mode(V4SI)));
typedef unsigned u16qi __attribute__((__mode__(__V16QI__)));
typedef float v2df __attribute__((mode(V2DF)));
typedef int v2ti __attribute__((mode(V2TI)));
typedef int v8byte __attribute__((mode(V8byte)));
typedef int i4 __attribute__((vector_size(16)));
typedef i4 l2 __attribute__((mode(DI)));
typedef i4 s2 __attribute__((mode(V2SI)));
typedef long gl2 __attribute__((vector_size(16)));
typedef int gs2 __attribute__((vector_size(8)));
static_assert(same<v4si, i4>::value);
static_assert(same<v2df, double __attribute__((vector_size(16)))>::value);
static_assert(same<u16qi, unsigned char __attribute__((vector_size(16)))>::value);
static_assert(same<v8byte, signed char __attribute__((vector_size(8)))>::value);
static_assert(same<l2, gl2>::value);
static_assert(same<s2, gs2>::value);

int main() {
  std::printf("%zu %zu %zu %zu %zu\n", sizeof(v4si), sizeof(u16qi), sizeof(v2df), sizeof(v2ti),
              sizeof(v8byte));
  v4si a = {1, 2, 3, 4};
  v4si b = a * 2 - 5;
  u16qi c = {};
  c[3] = 255;
  v2df d = {0.5, 1.0};
  d = d + 1.0;
  std::printf("%d %d %d %.1f\n", b[3], b[1] * 4, (int)c[3], d[0]);
  l2 x = {1, 2};
  s2 y = {2, 4};
  std::printf("%ld %d %d\n", x[1], y[0], y[1]);
}
