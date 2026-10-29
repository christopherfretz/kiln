//@ expect: ast
//@ oracle: clang
// [temp.dep.type]/9: `decltype` of a type-dependent expression
template <class T> void f(T t) { decltype(t) u = t; }
template <class T> struct W { decltype(T() + 1) v; decltype(sizeof(T)) n; };
