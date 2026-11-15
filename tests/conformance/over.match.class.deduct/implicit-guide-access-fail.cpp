// [over.match.class.deduct]/1.1, [class.access.general]/7: the class's own
// access does not reach a private member of an unrelated class, so the
// guide's substitution fails as the constructor's declaration would.
//@ expect: compile-fail
//@ diag: 14:3 error "no viable constructor or deduction guide for deduction of template arguments of 'C'"
//@ diag: 12:3 note "candidate template ignored: substitution failure [with T = int]: 'type' is a private member of 'Priv<int>'"
//@ diag: 11:27 note "candidate function template not viable: requires 1 argument, but 2 were provided"
template <class T> class Priv {
  using type = T;
};
template <class T> struct C {
  C(T, typename Priv<T>::type);
};
C c(1, 1);
