//@ expect: run
//@ strict-diags
//@ args: -std=gnu++26
//@ stdout: 12 12 24 20
//@ stdout: 60 60 60 1
//@ stdout: 28 1 8
//@ stdout: 16 16
// A variable length array type in a function template ([temp.inst],
// GNU extension): its bound names the specialization's parameter or
// local, so every expression that names the type (`sizeof(int[w])`,
// a typedef, a cast) is rebuilt in each specialization, even when the
// array type does not depend on a template parameter. Before, the
// non-dependent `sizeof(int[w])` was shared with the pattern and code
// generation failed with "reference to an unemitted local variable".
// The bound is evaluated once, where the type is declared (a typedef),
// not where it is used. Found running GCC's C++ testsuite.
extern "C" int printf(const char*, ...);

template <bool> static int discarded(int w) {
  (void)sizeof(int[w]);
  return int(sizeof(int[w]));
}
template <class T> int type_and_object(int w) {
  T a[w];
  return int(sizeof a);
}
template <class> int dependent_elem(int w) { return int(sizeof(long[w][3]) / 3 + sizeof(int[w + 2])); }

template <int N> int nested(int a, int b) {
  int m[a][N][b];
  return int(sizeof m + sizeof(int[a][N][b]) + sizeof(m[0]) * a);
}

static int calls;
static int next(int v) { ++calls; return v; }
template <class T> int typedef_once(int w) {
  typedef T A[next(w)];
  w = 100;
  return int(sizeof(A) + 0 * sizeof(A));
}

template <class T> int in_generic_lambda(int w) {
  auto l = [&](auto x) { return int(sizeof(T[w])) + int(sizeof x) - int(sizeof x); };
  return l(1) + l('c');
}

int main() {
  printf("%d %d %d %d\n", discarded<false>(3), discarded<true>(3), type_and_object<double>(3),
         dependent_elem<int>(1));
  printf("%d %d %d %d\n", nested<1>(3, 5) / 3, nested<5>(1, 3) / 3, nested<3>(5, 1) / 3,
         int(sizeof(int[1]) == 4));
  int t = typedef_once<int>(7);
  int c = calls;
  int u = typedef_once<char>(8);
  printf("%d %d %d\n", t, c, u);
  printf("%d %d\n", in_generic_lambda<int>(2), in_generic_lambda<short>(4));
}
