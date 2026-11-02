//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 17:9 error "pack declaration outside of template"
//@ diag: 18:12 error "pack declaration outside of template"
//@ diag: 21:19 error "multiple packs in structured binding declaration"
//@ diag: 21:12 note "previous binding pack specified here"
//@ diag: 25:8 error "elements, but 5 names were provided"
//@ diag: 28:9 note "in instantiation of function template specialization 'more<P>' requested here"
// [dcl.struct.bind]/1 (C++26 P1061): an sb-identifier `...xs` declares a
// structured binding pack. At most one sb-identifier is a pack, and only a
// templated entity declares one (clang 22); the pack counts as one name
// against the number of elements, so it may be empty but the others may
// not outnumber the elements (diagnosed at the `[`, at instantiation).
struct P { int x, y, z; };
void f(P p) {
  auto [...xs] = p;
  auto [a, ...ys] = p;
}
template <class T> int g(T t) {
  auto [a, ...xs, ...ys] = t;
  return 0;
}
template <class T> int more(T t) {
  auto [a, b, c, d, ...e] = t;
  return sizeof...(e);
}
int u = more(P{1, 2, 3});
