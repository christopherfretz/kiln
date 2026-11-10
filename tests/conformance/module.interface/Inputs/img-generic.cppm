// The interface of image-generic-run.cpp (WP-1132): `_Generic` and
// `__builtin_choose_expr` in exported inline, constexpr and template
// bodies, carried by the declaration image. (The type-name control form
// is covered by kiln_driver's modules_bmi test only: clang 19 crashes
// serializing it.)
export module imgg;
export namespace ig {
// An expression control; type and `default` associations.
inline int kind(int x) { return _Generic(x, int: 1, long: 2, default: 3); }
inline int kind(double x) { return _Generic(x, int: 1, long: 2, default: 3); }
// The selected association uses the parameter.
constexpr int twice(int x) { return _Generic(x + 0, int: x * 2, default: -1); }
// The selection is an lvalue: its value category is the selected one's.
inline int &pick(int &a, long &b) { return _Generic(a, int: a, long: b); }
// A dependent control, instantiated by the importer.
template <class T> constexpr int tag(T v) {
  return _Generic(v, int: 1, char: 2, double: 3, default: 0);
}
// A dependent association type.
template <class T> constexpr int is(int x) { return _Generic(x, T: 1, default: 0); }
// A dependent lvalue selection.
template <class T> T &first(T &a, T &b) { return _Generic(a, int: a, default: b); }

// `__builtin_choose_expr`: the condition, both arms, the chosen one.
constexpr int choose(int x) { return __builtin_choose_expr(sizeof(int) >= 2, x + 1, x - 1); }
constexpr unsigned long choose_size() { return sizeof(__builtin_choose_expr(0, 'a', 1L)); }
inline int &choose_lv(int &a, int &b) { return __builtin_choose_expr(1, a, b); }
// A dependent condition, chosen at instantiation.
template <int N> constexpr int choose_t(int x) { return __builtin_choose_expr(N, x * 10, x); }
} // namespace ig
