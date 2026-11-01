// [dcl.struct.bind]/1, [dcl.pre]/6 (C++26, P0609R3): an attribute may
// appertain to each structured binding. WP-185 feature probe.
//@ expect: compile-ok
//@ strict-diags
struct P { int x, y; };
int f(P p) {
  auto [a [[maybe_unused]], b] = p;
  return b;
}
