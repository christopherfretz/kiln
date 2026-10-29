//@ expect: ast
//@ oracle: clang
// [temp.dep.constexpr]/2: value-dependent uses of non-type parameters
template <int N> int next() { return N + 1; }
template <int N> struct A { int arr[N]; int twice[N * 2]; };
template <class T, int N> T sum(T (&a)[N]) { return a[0]; }
