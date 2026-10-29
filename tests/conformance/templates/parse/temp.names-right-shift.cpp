//@ expect: ast
//@ oracle: clang
// [temp.names]/4: `>>` closes two template argument lists
template <class T> struct A {};
template <class T> struct B { A<A<T>> x; A<A<A<T>>> *y; };
template <class T> struct C { A<A<T> > z; };
