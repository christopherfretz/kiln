//@ expect: ast
//@ oracle: clang
// [temp.arg.general]: dependent class template specializations
template <class T, int N> struct A {};
template <class T> struct B { A<T, 3> a; A<T *, sizeof(T)> b; };
template <class T> A<T, 1> *f(A<T, 2> &);
