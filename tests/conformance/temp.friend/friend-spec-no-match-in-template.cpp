// [temp.friend]/1, [temp.expl.spec]/12 and [temp.res.general]/6: a
// friend declaration naming a function template specialization whose
// type and template arguments depend on no template parameter is matched
// where the class template is defined; when no function template
// matches it the template is ill-formed whether or not it is ever
// instantiated (clang 22 "no function template matches function template
// specialization", at the definition only; GCC 16.1 diagnoses it at
// instantiation, and at the definition only with a default argument). A
// dependent one is matched when instantiated. The accepted forms are
// temp.friend/friend-spec-in-template-run.cpp.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 31:18 error "no function template matches function template specialization 'foo'"
//@ diag: 26:23 note "candidate template ignored: could not match 'void ()' against 'void (int)'"
//@ diag: 34:15 error "no function template matches function template specialization 'g'"
//@ diag: 28:25 note "candidate template ignored: failed template argument deduction"
//@ diag: 37:18 error "no function template matches function template specialization 'foo'"
//@ diag: 26:23 note "candidate template ignored: could not match 'void ()' against 'void (int)'"
//@ diag: 41:15 error "no function template matches function template specialization 'g'"
//@ diag: 43:9 note "in instantiation of template class 'B4<int>' requested here"
//@ diag: 28:25 note "candidate template ignored: failed template argument deduction"

struct A {
  template <int> void foo();
};
template <class T> void g(T);

template <int> struct B1 {
  friend void A::foo<0>(int);
};
template <class T> struct B2 {
  friend void g<>(int, int);
};
template <class T> struct B3 {
  friend void A::foo<1>(int);
};
B3<int> b3;
template <class T> struct B4 {
  friend void g<>(T, T);
};
B4<int> b4;
