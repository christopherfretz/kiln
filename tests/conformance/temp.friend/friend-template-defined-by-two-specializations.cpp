// [temp.inject]/1, [basic.def.odr]/1: a friend function template defined
// in a class template is defined by each instantiation of the class; two
// instantiations defining the same template (here of two class templates)
// are a redefinition, reported when the second one is instantiated, and
// only then: a class template that is never instantiated defines nothing.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:42 error "redefinition of 'alloc'"
//@ diag: 24:17 note "in instantiation of template class 'two<int>' requested here"
//@ diag: 16:42 note "previous definition is here"
template<class T> struct slot {
  template<class U> friend constexpr int alloc(slot<T>, U);
};
template<class T> struct one {
  template<class U> friend constexpr int alloc(slot<T>, U) { return 1; }
};
template<class T> struct two {
  template<class U> friend constexpr int alloc(slot<T>, U) { return 2; }
};
template struct one<int>;
template struct two<long>;
static_assert(alloc(slot<int>{}, 0) == 1, "");
template struct two<int>;
