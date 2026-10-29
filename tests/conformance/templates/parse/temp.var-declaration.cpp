//@ expect: ast
//@ oracle: clang
// [temp.pre]/1: variable templates
template <class T> constexpr int zero = 0;
template <class T> T dflt = T();
template <class T> struct S { template <class U> static constexpr bool same = false; };
