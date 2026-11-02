// [expr.prim.lambda.closure]/3, [temp.variadic]/5, [expr.prim.fold]: a fold
// inside a lambda with an explicit template-parameter-list, at run time.
// The fold names the lambda's own pack, the enclosing function template's
// parameter pack (captured by reference through `[&]`), and both at once
// (the libc++ `layout_stride::mapping::operator()` form).
//@ expect: run
//@ stdout: 6 6 140 9
extern "C" int printf(const char *, ...);
template <unsigned long... I> struct seq {};
template <class T> T own() { return []<class... Ts>(Ts... ts) { return (ts + ...); }(T(1), T(2), T(3)); }
struct M {
  long s[3] = {10, 20, 30};
  template <class... Idx> long op(Idx... idx) const {
    return [&]<unsigned long... P>(seq<P...>) {
      return ((static_cast<long>(idx) * s[P]) + ... + long(0));
    }(seq<0, 1, 2>());
  }
};
template <class... Idx> long outer_only(Idx... idx) {
  return [&]<class U>(U u) { return (idx + ... + u); }(0L);
}
int main() {
  auto g = []<class... Ts>(Ts... ts) { return (ts + ...); };
  printf("%d %d %ld %ld\n", g(1, 2, 3), own<int>(), M{}.op(1, 2, 3), outer_only(2, 3, 4));
}
