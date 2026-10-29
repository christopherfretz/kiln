//@ expect: ast
//@ oracle: clang
// [temp.dep.type], [class.derived]: dependent base classes
template <class T> struct Base {};
template <class T> struct D1 : Base<T> {};
template <class T> struct D2 : T {};
template <class... Ts> struct D3 : Ts... {};
