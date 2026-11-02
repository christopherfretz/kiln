// [expr.prim.pack.index], [dcl.type.pack.index]: a function parameter pack
// or template parameter pack indexed inside a generic lambda or a member
// template by that inner template's own parameter: the index stays
// dependent when the enclosing template is instantiated and is resolved
// when the inner one is.
template <int N> struct ic { static constexpr int value = N; };
template <class... Ts> constexpr int g1(Ts... ts) {
  return [&]<int I>() { return ts...[I]; }.template operator()<1>();
}
template <class... Ts> constexpr int g2(Ts... ts) {
  return [&]<int... Is>() { return (ts...[Is] + ...); }.template operator()<0, 2>();
}
template <class... Ts> constexpr long g3(Ts... ts) {
  return [&]<int I>() {
    Ts...[I] v = ts...[I];
    return v;
  }.template operator()<1>();
}
template <class... Ts> constexpr int g4(Ts... ts) {
  return [&](auto i) { return ts...[decltype(i)::value]; }(ic<2>());
}
static_assert(g1(1, 20, 3) == 20);
static_assert(g2(1, 20, 300) == 301);
static_assert(g3(7, 2L) == 2L);
static_assert(g4(1, 2, 30) == 30);
template <class... Ts> struct tl {
  template <int I> using at = Ts...[I];
  template <int I> static constexpr long get(Ts... ts) { return ts...[I]; }
};
static_assert(__is_same(tl<int, long>::at<1>, long));
static_assert(tl<int, long>::get<1>(3, 4L) == 4L);
