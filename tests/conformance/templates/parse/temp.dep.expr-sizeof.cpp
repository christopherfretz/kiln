//@ expect: ast
//@ oracle: clang
//@ requires: clang>=22
// [temp.dep.constexpr]/3: `sizeof` and `alignof` of dependent types
template <class T> unsigned long s() { return sizeof(T) + alignof(T); }
template <class T> unsigned long e(T t) { return sizeof t; }
