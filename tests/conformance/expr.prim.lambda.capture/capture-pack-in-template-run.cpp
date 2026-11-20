// [expr.prim.lambda.capture]/17-18, [temp.variadic]/5.10: a
// simple-capture pack `ts...`/`&ts...` and an init-capture pack
// `...ys = ts` in a function template, in a generic lambda (its own
// pack kept until its call operator is instantiated), in a lambda nested
// in another one that captured the pack, and mixing both levels'
// packs in one capture list.
//@ expect: run
//@ std: c++26
//@ stdout: 18 9 25 5
#include <cstdio>
template <class... Ts> int f(Ts... ts) {
  auto g = [](auto... us) { return [us...] { return (us + ... + 0); }(); };
  return [&ts...] { return (ts + ... + 0); }() + g(ts...) +
         [=] { return [ts...] { return (ts * ... * 1); }(); }();
}
template <class... Ts> int k(Ts... ts) {
  return [...ys = ts] { return [ys...] { return (ys + ... + 0); }(); }();
}
template <class... Ts> int m(Ts... ts) {
  return [](auto... vs) {
    return [&](auto... ws) { return [vs..., ws...] { return (vs + ... + 0) * (ws + ... + 0); }(); }(vs...);
  }(ts...);
}
template <class... Ts> int n(Ts... ts) {
  int r = 0;
  [&](auto... us) { [&r, &ts..., us...] { r = sizeof...(ts) + sizeof...(us); }(); }(1, 2);
  return r;
}
int main() { std::printf("%d %d %d %d\n", f(1, 2, 3), k(4, 5), m(2, 3), n(1, 2, 3)); }
