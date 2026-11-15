// Debug info (WP-1445): a block-scope structured binding gets a
// DILocalVariable at the decomposed object's storage, as clang emits it:
// a member at a nonzero offset adds DW_OP_plus_uconst, a reference
// decomposition starts with DW_OP_deref, a bit-field is extracted with
// DW_OP_LLVM_extract_bits_{s,z}ext, an array element at its index, and a
// tuple-like binding is its hidden variable. The decomposed object itself
// has no variable. Bindings in range-for, if and switch initializers, a
// static local and a lambda capture.
//@ args: -g
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}
struct T2 {
  int a;
  long b;
  template <unsigned long I> auto get() const {
    if constexpr (I == 0) return a; else return b;
  }
};
template <> struct std::tuple_size<T2> { static constexpr unsigned long value = 2; };
template <> struct std::tuple_element<0, T2> { using type = int; };
template <> struct std::tuple_element<1, T2> { using type = long; };
struct P { int a; long b; };
struct B { int x : 3; unsigned y : 5; char c; unsigned long long z : 40; };
struct Base { int i; short s; };
struct Der : Base {};
P mk();
constexpr P kp{3, 4};
long f(P p, B bb, int n) {
  auto [x, y] = p;
  auto &[rx, ry] = p;
  const auto &[cx, cy] = mk();
  auto &&[fx, fy] = mk();
  int arr[3] = {1, 2, 3};
  auto [a0, a1, a2] = arr;
  auto [bx, by, bc, bz] = bb;
  T2 t{1, 2};
  auto [t0, t1] = t;
  Der dd{};
  auto [di, ds] = dd;
  long s = 0;
  P ps[2] = {};
  for (auto [u, v] : ps)
    s += u + v;
  if (auto [i1, i2] = p; i1)
    s += i2;
  switch (auto [w1, w2] = p; w1) {
  default:
    s += w2;
  }
  auto l = [x, &ry] { return x + ry; };
  static auto [sx, sy] = kp;
  return x + y + rx + ry + cx + cy + fx + fy + a0 + a1 + a2 + bx + by + bc + bz +
         t0 + t1 + n + di + ds + s + l() + sx + sy;
}
