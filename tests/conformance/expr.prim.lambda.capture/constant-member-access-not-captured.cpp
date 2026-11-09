// [basic.def.odr]/4: the potential results of a class member access
// `e.m` include those of `e`, so `ca.v` with `ca` a constexpr object is
// not an odr-use of `ca` and a by-copy default capture does not capture
// it: the closure is empty (WP-1144).
//@ expect: run

struct A { int v; };

int main() {
  constexpr A ca{4};
  auto l = [=] { return ca.v; };
  static_assert(sizeof(l) == 1);
  return l() - 4;
}
