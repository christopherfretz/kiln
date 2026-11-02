// [temp.variadic]/7, [expr.prim.fold]: a fold in a generic lambda's body
// that mixes the enclosing function template's parameter pack with the
// lambda's own template parameter pack requires equal lengths; the
// mismatch is diagnosed when the call operator is instantiated.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:63 error "pack expansion contains parameter pack 'P' that has a different length (3 vs. 2) from outer parameter packs"
//@ diag: 12:10 note "in instantiation of function template specialization 'f(int, int, int)::(anonymous class)::operator()<0, 1>' requested here"
//@ diag: 14:10 note "in instantiation of function template specialization 'f<int, int, int>' requested here"
template <int... I> struct seq {};
template <class... Idx> long f(Idx... idx) {
  return [&]<int... P>(seq<P...>) { return ((idx * long(P)) + ... + 0L); }(seq<0, 1>());
}
long g = f(1, 2, 3);
