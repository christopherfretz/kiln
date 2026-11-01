// [dcl.init.aggr]/3.1 with the GNU array-designator extensions clang
// accepts in C++: an index designator `[i] =` (a C99 extension) and a
// range designator `[a ... b] =` initializing every element of the range
// from the same clause. Designators fix the array bound, override earlier
// initializers, apply in constant evaluation and in templates.
//@ expect: run
//@ args: -Wno-c99-designator -Wno-initializer-overrides
struct P { int x, y; };
int g1[6] = {[1 ... 3] = 7, [5] = 9};
int g2[] = {[2 ... 4] = 1, 2};
P g3[4] = {[0 ... 1] = {1, 2}, [3].y = 5};
constexpr int c1[5] = {[0 ... 4] = 3, [2] = 8};
static_assert(c1[0] == 3 && c1[2] == 8 && c1[4] == 3);
static_assert(sizeof(g2) == 6 * sizeof(int));
template <int N> struct A { int v[N] = {[0 ... N - 1] = N}; };
template <class T, int I> constexpr T pick() {
  T a[4] = {[I] = T(5), [I + 1 ... 3] = T(2)};
  return a[I] + a[3];
}
static_assert(pick<int, 1>() == 7);
static_assert(A<3>{}.v[2] == 3);
int f(int k) {
  int a[5] = {[1 ... 3] = k, [4] = k + 1};
  int m[2][3] = {[1] = {[0 ... 2] = k}, [0][1] = 4};
  return a[0] + a[1] + a[3] + a[4] + m[1][2] + m[0][1];
}
int main() {
  if (g1[0] != 0 || g1[1] != 7 || g1[3] != 7 || g1[4] != 0 || g1[5] != 9) return 1;
  if (g2[2] != 1 || g2[4] != 1 || g2[5] != 2) return 2;
  if (g3[1].y != 2 || g3[2].x != 0 || g3[3].y != 5) return 3;
  if (f(2) != 0 + 2 + 2 + 3 + 2 + 4) return 4;
  A<4> a;
  if (a.v[3] != 4) return 5;
  return 0;
}
