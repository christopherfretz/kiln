// [expr.prim.lambda.capture]/18, [dcl.struct.bind]/1 (P1061): a
// structured binding pack captured by a capture pack, directly, from a
// lambda nested in one capturing it by copy, and next to a generic
// lambda's own pack.
//@ expect: run
//@ std: c++26
//@ requires: clang>=22
//@ stdout: 9 3 6
#include <cstdio>
#include <tuple>
template <class T> int c(T t) {
  auto [...xs] = t;
  return [xs...] { return (xs + ... + 0); }() + [&xs...] { return int(sizeof...(xs)); }();
}
template <class T> int d(T t) {
  auto [...xs] = t;
  return [=] { return [xs...] { return (xs + ... + 0); }(); }();
}
template <class T> int e(T t) {
  auto [...xs] = t;
  return [&](auto... ys) { return [xs..., ys...] { return (xs + ... + 0) + (ys + ... + 0); }(); }(xs...);
}
int main() { std::printf("%d %d %d\n", c(std::tuple{1, 2, 3}), d(std::tuple{1, 2}), e(std::tuple{1, 2})); }
