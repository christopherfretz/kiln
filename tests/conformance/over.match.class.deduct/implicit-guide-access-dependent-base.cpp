// [over.match.class.deduct]/1.1: a protected member of a class the
// constructor's class does not derive from stays inaccessible; the error is
// the constructor declaration's, in the class that deduction selected.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:27 error "'prot' is a protected member of 'Other<int>'"
//@ diag: 17:3 note "in instantiation of template class 'C<int>' requested here"
//@ diag: 12:9 note "declared protected here"
template <class T> struct Base {};
template <class T> struct Other {
protected:
  using prot = T;
};
template <class T> struct C : Base<T> {
  C(T, typename Other<T>::prot);
};
C c(1, 1);
