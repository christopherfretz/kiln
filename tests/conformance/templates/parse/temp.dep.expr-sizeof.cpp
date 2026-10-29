//@ expect: ast
//@ oracle: clang
// [temp.dep.constexpr]/3: `sizeof` and `alignof` of dependent types
template <class T> unsigned long s() { return sizeof(T) + alignof(T); }
template <class T> unsigned long e(T t) { return sizeof t; }
