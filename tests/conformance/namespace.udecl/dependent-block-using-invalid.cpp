// [namespace.udecl]/8: a block-scope dependent using-declarator is checked
// at instantiation; one that names no enumerator is an error there.
// WP-179.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:12 error "using declaration cannot refer to class member"
//@ diag: 15:9 note "in instantiation of function template specialization 'g<S>' requested here"

struct S { int x; enum { Z }; };
template<class T> int g() {
  using T::Z;
  using T::nope;
  return Z;
}
int b = g<S>();
