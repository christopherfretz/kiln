//@ expect: ast
//@ oracle: clang
// [dcl.decl.general]/4: a trailing requires-clause
template <class T> concept C = true;
template <class T> void f(T) requires C<T> {}
template <class T> int g(T t) requires C<T> || (sizeof(T) == 4) { return 1; }
