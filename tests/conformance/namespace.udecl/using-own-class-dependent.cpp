// [namespace.udecl]/3: in a class template, 'using Col<T>::operator();'
// names a member of the current instantiation; clang accepts it (PR14768).
// From clang's CXX/dcl.dcl/basic.namespace/namespace.udecl/p10.cpp. Found by
// the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, a dependent using-declaration naming the current instantiation is rejected
//@ expect: compile-ok
template <typename T> class Mat;
template <typename T> class Col : public Mat<T> {
  using Mat<T>::operator();
  using Col<T>::operator();
  void operator()();
};
