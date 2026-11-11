// Header of module `imgh` (Inputs/img-hdr.cppm), #included in its purview
// (spec/modules.md §11.11 "File table"): functions, classes and function
// templates whose locations lie in this file, not in the unit's.
#define IMGH_TWICE(x) ((x) + (x))
#define IMGH_GETTER(name, v)                                                  \
  constexpr int get_##name() { return v; }
namespace hh {
constexpr int add(int a, int b) { return a + b; }
int sub(int a, int b);
inline int sub(int a, int b) { return a - b; }
struct Point {
  int x, y;
  constexpr int sum() const { return x + y; }
  int scaled(int k) const { return IMGH_TWICE(k) * x; }
};
template <class T> constexpr T twice(T t) { return IMGH_TWICE(t); }
template <class T, int N> constexpr T times(T t) { return t * N; }
IMGH_GETTER(seven, 7)
IMGH_GETTER(nine, IMGH_TWICE(4) + 1)
} // namespace hh
