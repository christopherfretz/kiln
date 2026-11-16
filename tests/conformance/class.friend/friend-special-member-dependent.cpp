// [temp.friend]/1: friend constructors of a class template's specialization
// and of a dependent class. kiln accepts them; a dependent one grants no
// access yet (spec/sema-class.md). `template<class U> friend X<U>::X(U);`
// is valid ([temp.friend]/1, GCC 16.1) but clang 22 rejects it ("friends
// can only be classes or functions"). Found running GCC's C++ testsuite
// (WP-1466).
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
//@ xfail(clang): clang 22 rejects a friend constructor template of a class template
template<class T> struct X { X(T); };
template<class T> struct Y {
  template<class U> friend X<U>::X(U);
  friend X<int>::X(int);
};
Y<char> y;
struct Q { typedef int t; };
template<class T> struct Z { Z(typename T::t); };
template<class T> struct W { friend Z<T>::Z(typename T::t); };
W<Q> w;
