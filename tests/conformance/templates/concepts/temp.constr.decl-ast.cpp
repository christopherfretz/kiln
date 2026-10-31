//@ expect: ast
//@ oracle: clang
// [temp.param]/4, [temp.constr.decl]: a type-constraint becomes the
// parameter's immediately-declared constraint; a trailing requires-clause
// follows the parameters.
template <class T> concept Big = sizeof(T) > 4;
template <Big T> int f(T t) requires (sizeof(T) < 16) { return 1; }
template <class T> requires Big<T> struct S { T v; };
