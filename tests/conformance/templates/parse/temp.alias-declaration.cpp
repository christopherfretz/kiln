//@ expect: ast
//@ oracle: clang
// [temp.alias]/1: alias templates
template <class T> using Ptr = T *;
template <class T> using Ref = const T &;
template <class... Ts> using Fn = void (*)(Ts...);
