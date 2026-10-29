//@ expect: ast
//@ oracle: clang
// [temp.param]/9: a non-type parameter whose type names an earlier parameter
template <class T, T V> struct Const { T get() { return V; } };
template <class T, T *P> struct PtrC {};
template <auto V> struct Auto {};
