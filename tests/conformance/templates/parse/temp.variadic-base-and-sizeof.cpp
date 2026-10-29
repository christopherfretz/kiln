//@ expect: ast
//@ oracle: clang
// [temp.variadic]/5: `sizeof...` of a template parameter pack in a class
template <class... Ts> struct Tuple { static const unsigned long size = sizeof...(Ts); };
template <int... Ns> struct Seq { int arr[sizeof...(Ns)]; };
