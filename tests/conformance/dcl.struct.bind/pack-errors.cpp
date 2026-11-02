//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 16:8 error "non-class, non-array type 'int *'"
//@ diag: 19:9 note "in instantiation of function template specialization 'n<int *>' requested here"
//@ diag: 24:23 error "pack expansion contains parameter packs 'xs' and 'ts' that have different lengths (2 vs. 3)"
//@ diag: 26:11 note "in instantiation of function template specialization 'mm<int, int, int>' requested here"
//@ diag: 29:10 error "invalid index 5 for pack"
//@ diag: 31:9 note "in instantiation of function template specialization 'idx<P>' requested here"
// [dcl.struct.bind]/1 (C++26 P1061) at instantiation: a pack in a type
// that is not decomposable, a binding pack expanded with a pack of another
// length ([temp.variadic]/8), and an out-of-range pack index
// ([expr.prim.pack.index]/2); uses of a failed pack are not diagnosed again.
struct P { int x, y, z; };
template <class T> int n(T t) {
  auto [...xs] = t;
  return (xs + ...);
}
int v = n((int*)nullptr);
struct Q { int a, b; };
template <class... Ts> int mm(Ts... ts) {
  Q q{1, 2};
  auto [...xs] = q;
  return ((xs * ts) + ...);
}
int bad = mm(1, 2, 3);
template <class T> int idx(T t) {
  auto [...xs] = t;
  return xs...[5];
}
int w = idx(P{1, 2, 3});
