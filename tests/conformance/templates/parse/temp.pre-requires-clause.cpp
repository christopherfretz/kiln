//@ expect: ast
//@ oracle: clang
// [temp.pre]/1: a requires-clause after the template-parameter-list
template <class T> concept C = true;
template <class T> requires C<T> struct A {};
template <class T> requires C<T> && (sizeof(T) > 1) void f(T);
template <class T> requires true int g() { return 0; }
