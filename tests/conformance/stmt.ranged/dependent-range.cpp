// [stmt.ranged] in templates ([temp.res], [temp.inst]): a dependent range
// is desugared at instantiation, for arrays, class ranges and dependent
// loop variable types; a non-dependent range in a template is desugared
// in the definition.
//@ expect: run
//@ stdout: 6 15 3 10
extern "C" int printf(const char*, ...);

template <class C> int sum(const C& c) {
  int s = 0;
  for (auto&& x : c) s += x;
  return s;
}
template <class T, int N> T sum_typed(T (&a)[N]) {
  T s = 0;
  for (T x : a) s += x;
  return s;
}
template <class T> int count() {
  int a[3] = {};
  int n = 0;
  for (T x : a) n += 1 + int(x);
  return n;
}
struct V {
  int d[3] = {4, 5, 6};
  const int* begin() const { return d; }
  const int* end() const { return d + 3; }
};
template <class T> constexpr T csum(const T (&a)[4]) {
  T s = 0;
  for (const T& x : a) s += x;
  return s;
}
constexpr int four[] = {1, 2, 3, 4};
static_assert(csum(four) == 10);

int main() {
  int a[] = {1, 2, 3};
  int b[] = {1, 2, 3, 4};
  printf("%d %d %d %d\n", sum(a), sum(V{}), count<long>(), sum_typed(b));
}
