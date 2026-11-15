//@ expect: run
//@ std: c++17
//@ strict-diags
//@ stdout: 1 4 10 40 5 8
//@ stdout: 3 2.5 1 2
// GNU extension (clang `checkVectorDecomposition` /
// `checkComplexDecomposition`, clang 19 and 22): a GNU vector type
// decomposes into its elements `e[i]` and a complex type into `__real e`
// and `__imag e`, before the tuple-like and data-member cases; a binding
// of a reference decomposition writes through to the vector element.
// Found running GCC's C++ testsuite, which accepts the same.
extern "C" int printf(const char *, ...);
typedef int V __attribute__((vector_size(4 * sizeof(int))));
typedef float E __attribute__((ext_vector_type(2)));
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;

int main() {
  V v = {1, 2, 3, 4};
  auto [a, b, c, d] = v;
  auto &[p, q, r, s] = v;
  p = 10;
  s = 40;
  const V cv = {5, 6, 7, 8};
  auto &[ca, cb, cc, cd] = cv;
  static_assert(same<decltype(ca), const int>);
  printf("%d %d %d %d %d %d\n", a, d, v[0], v[3], ca, cd);
  _Complex double z = 1.5;
  __imag__ z = 2.5;
  auto &[re, im] = z;
  re = 3.0;
  static_assert(same<decltype(re), double>);
  E e = {1.0f, 2.0f};
  auto [x, y] = e;
  printf("%g %g %g %g\n", __real__ z, im, x, y);
}
