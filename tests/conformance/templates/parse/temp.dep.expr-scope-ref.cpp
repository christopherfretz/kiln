//@ expect: ast
//@ oracle: clang
// [temp.dep.expr]/3: a qualified-id with a dependent nested-name-specifier
template <class T> int f() { return T::value; }
template <class T> int g() { return T::nested::value + 1; }
template <class T> void h() { T::fn(); }
