//@ expect: ast
//@ oracle: clang
// [temp.over.link]: redeclarations of function templates
template <class T> void f(T);
template <class T> void f(T);
template <class T> void f(T x) {}
