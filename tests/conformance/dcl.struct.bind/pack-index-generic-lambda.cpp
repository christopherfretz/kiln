//@ requires: clang>=22
// [expr.prim.pack.index], [dcl.struct.bind]/1 (P1061): a structured binding
// pack indexed inside a generic lambda by the lambda's own template
// parameter: the index stays dependent when the enclosing function is
// instantiated and is resolved when the lambda's call operator is.
struct P { int x, y, z; };
template <int N> struct ic { static constexpr int value = N; };
template <class T> constexpr int f1(T t) {
  auto [...xs] = t;
  return [&]<int I>() { return xs...[I]; }.template operator()<1>();
}
template <class T> constexpr int f2(T t) {
  auto [...xs] = t;
  return [&]<int... Is>() { return (xs...[Is] + ...); }.template operator()<0, 2>();
}
template <class T> constexpr int f3(T t) {
  auto [a, ...xs] = t;
  return [&](auto i) { return a + xs...[decltype(i)::value]; }(ic<1>());
}
static_assert(f1(P{1, 2, 3}) == 2);
static_assert(f2(P{1, 2, 3}) == 4);
static_assert(f3(P{1, 2, 3}) == 4);
