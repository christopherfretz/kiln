//@ expect: ast
//@ oracle: clang
//@ requires: clang>=22
// [temp.param]/4: non-type template parameters and their defaults
template <int N> struct A {};
template <int N, bool F = true, unsigned long M = sizeof(int)> struct B {};
template <class T, T V> struct C {};
template <const char *P, int &R> struct D {};
