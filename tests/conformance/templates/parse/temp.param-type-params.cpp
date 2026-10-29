//@ expect: ast
//@ oracle: clang
// [temp.param]/2-3: type parameters (`class`, `typename`), defaults
template <class T> struct A {};
template <typename T, class U = int> struct B {};
template <class T = char, typename = long> struct C {};
