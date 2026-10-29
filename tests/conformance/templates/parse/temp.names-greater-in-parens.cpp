//@ expect: ast
//@ oracle: clang
// [temp.names]/4: a `>` inside parentheses in a template argument
template <class T, bool B> struct A {};
template <class T> struct C { A<T, (3 > 2)> x; A<T, (sizeof(T) > 1)> y; };
