// [dcl.struct.bind]/1, [expr.prim.lambda.capture]/18: when the
// declaration of a structured binding pack fails in an instantiation,
// a capture pack naming it fails with it, with no further diagnostic.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ requires: clang>=22
//@ diag: 11:8 error "cannot bind non-class, non-array type 'int'"
//@ diag: 14:9 note "in instantiation of function template specialization 'c<int>' requested here"
template <class T> int c(T t) {
  auto [...xs] = t;
  return [xs...] { return (xs + ... + 0); }();
}
int d = c(5);
