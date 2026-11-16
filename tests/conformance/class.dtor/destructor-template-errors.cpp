// [class.dtor]/1: a destructor shall not be a member template. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 8:20 error "destructor cannot be declared as a template"
//@ diag: 11:45 error "destructor cannot be declared as a template"
struct Foo {
  template <int I> ~Foo() {}
};
template <class T> struct A { ~A(); };
template <class T> template <class U> A<T>::~A() {}
// Valid: destructors of a class template, a member class template and a
// partial specialization, defined out of class.
template <class T> struct B { ~B(); template <class U> struct M { ~M(); }; };
template <class T> B<T>::~B() {}
template <class T> template <class U> B<T>::M<U>::~M() {}
template <class T, class U> struct P {};
template <class T> struct P<T, int> { ~P(); };
template <class T> P<T, int>::~P() {}
B<int> b;
B<int>::M<char> m;
P<char, int> p;
