//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 18:26 error "no member named 'operator int *' in 'S<int>'"
//@ diag: 22:41 note "in instantiation of member function 'S<int>::g' requested here"
//@ diag: 19:32 error "no member named 'operator int *' in 'B<int>'"
//@ diag: 22:49 note "in instantiation of member function 'S<int>::h' requested here"
//@ diag: 20:32 error "no member named 'operator long' in 'S<int>'"
//@ diag: 22:57 note "in instantiation of member function 'S<int>::k' requested here"
// [temp.dep.expr]/3, [temp.res.general]/9: a conversion-function-id naming a
// dependent type is looked up only at instantiation, so none of these is
// diagnosed where the template is defined; at instantiation the lookup
// finds nothing (`operator int` exists, `operator int *` and `operator
// long` do not) and clang's `no member named` is reported (WP-1126).
template <class T> struct B { operator T() const { return T(); } };
template <class T> struct S : B<T> {
  int i() const { return operator T(); }
  int g() const { return operator T *() != nullptr; }
  int h() const { return B<T>::operator T *() != nullptr; }
  int k() const { return this->operator decltype(T() + 0L)(); }
};
int main() { S<int> s; return s.i() + s.g() + s.h() + s.k(); }
