// [expr.prim.lambda.capture]/17, [temp.variadic]/5: a lambda nested in a
// pack expansion captures an element of the enclosing function parameter
// pack, explicitly or implicitly; a generic lambda keeps the outer pack
// unexpanded until its own pack is known, and expands both together.
//@ expect: compile-ok
//@ xfail(gcc): GCC 16.1 rejects a capture of a pack element inside a fold over the pack ("parameter packs not expanded"; [expr.prim.lambda.capture]/17, [temp.variadic]/5)
constexpr int g(auto... xs) { return (xs + ... + 0); }
template <class... T> constexpr int nf(T... ts) {
  return [&] { return (... + [&] { return ts * 2; }()); }();
}
static_assert(nf(1, 2, 3) == 12);
template <class... T> constexpr int ng(T... ts) {
  return (... + [ts] { return ts; }());
}
static_assert(ng(1, 2, 3) == 6);
template <class... T> constexpr int nh(T... ts) {
  return (... + [=] { return ts; }());
}
static_assert(nh(1, 2, 3) == 6);
template <class... T> constexpr int nk(T... ts) {
  return [ts...] { return (... + [ts] { return ts; }()); }();
}
static_assert(nk(1, 2, 3) == 6);
// Outer pack used inside generic lambda, expanded inside the lambda body.
template <class... T> constexpr int a(T... ts) {
  return [](auto... us) { return g(us...); }(ts...);
}
static_assert(a(1, 2, 3) == 6);
// Outer pack and inner pack expanded together (keep mode).
template <class... T> constexpr int b(T... ts) {
  return [ts...](auto... us) { return g((ts * us)...); }(ts...);
}
static_assert(b(1, 2, 3) == 14);
// Nested expansion inside a kept expansion.
template <class... T> constexpr int c(T... ts) {
  return [&](auto... us) { return g(g(us, ts...)...); }(ts...);
}
static_assert(c(1, 2) == 9);
// Lambda expanded by an outer expansion.
template <class... T> constexpr int d(T... ts) {
  return g([ts] { return ts * 2; }()...);
}
static_assert(d(1, 2, 3) == 12);
// Nested lambda capturing an outer pack element inside an expansion.
template <class... T> constexpr int e(T... ts) {
  return g([=] { return [ts] { return ts; }(); }()...);
}
static_assert(e(1, 2, 3) == 6);
template <class... T> constexpr int f2(T... ts) {
  return [=] { return [ts...] { return g(ts...); }(); }();
}
static_assert(f2(1, 2, 3) == 6);
template <int... N> struct V { static constexpr int s = (N + ... + 0); };
struct A { int x, y, z; };
template <class... T> constexpr int ma(T... ts) {
  return g([ts](auto... us) { return g(us..., ts); }(1, 2)...);
}
static_assert(ma(10, 20) == 36);
template <class... T> constexpr int mb(T... ts) {
  return g([ts](auto... us) { int arr[] = {us..., ts}; return arr[0] + arr[sizeof...(us)]; }(1, 2)...);
}
static_assert(mb(10, 20) == 32);
template <int... M> constexpr int mc() {
  return g([]<int... K>() { return V<K..., M>::s; }.template operator()<1, 2>()...);
}
static_assert(mc<10, 20>() == 36);
template <class... T> constexpr int md(T... ts) {
  return g([ts](auto... us) { return A{us..., ts}.z; }(1, 2)...);
}
static_assert(md(10, 20) == 30);
template <class... T> constexpr int me(T... ts) {
  return g([ts](auto... us) { return (us + ... + 0) + ts; }(1, 2)...);
}
static_assert(me(10, 20) == 36);
