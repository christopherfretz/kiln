// [expr.alignof], codegen: the known alignment of an access through a
// `U*` in a function template instantiated with a typedef carrying GNU
// `__attribute__((aligned(32)))` is the canonical type's (clang
// instantiates from the canonical arguments): `load<D>`'s `*p` and its
// local `U` are 8-aligned, while a `D*` outside the template keeps the
// typedef's 32. Found running GCC's C++ testsuite (the sibling of
// `alignof` of a cv-qualified aligned array typedef).
//@ expect: ir
//@ std: c++20
typedef double D __attribute__((aligned(32)));
template <class U> U load(U* p) {
  U x = *p;
  return x;
}
double use(D* q) { return load<D>(q) + *q; }
