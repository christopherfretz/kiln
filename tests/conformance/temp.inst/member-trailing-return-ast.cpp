//@ expect: ast
//@ oracle: clang
// [temp.inst]/3, [dcl.fct]/2: the member declarations of an implicit class
// template instantiation keep a trailing return type as written: a member
// function template and a member function declared `auto f(U) -> T` dump as
// `auto (U) -> int` in the specialization, as clang's dump.
template <class T> struct A {
  template <class U> auto f(U) -> T;
  auto g(int) -> T;
  template <class U> auto h(U, T) -> T*;
};
A<int> a;
