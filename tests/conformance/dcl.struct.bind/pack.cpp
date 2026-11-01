//@ expect: compile-fail
//@ skip(clang): clang 19 does not parse structured binding packs (P1061)
//@ strict-diags
//@ diag: 12:9 error "a structured binding pack is not supported yet"
//@ diag: 13:12 error "a structured binding pack is not supported yet"
//@ diag: 15:39 error "a structured binding pack is not supported yet"
// [dcl.struct.bind]/1 (C++26 P1061): a structured binding pack `...xs`
// introduces a pack of bindings. kiln parses it and reports it as not yet
// supported.
struct P { int x, y, z; };
void f(P p) {
  auto [...xs] = p;
  auto [a, ...ys] = p;
}
template <class T> int g(T t) { auto [...zs] = t; return 0; }
