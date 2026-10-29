// [temp.spec.general]/5.1: an explicit instantiation definition shall
// appear at most once per specialization.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:17 error "duplicate explicit instantiation of 'A<int>'"
//@ diag: 8:17 note "previous explicit instantiation is here"
template <class T> struct A {};
template struct A<int>;
template struct A<int>;
