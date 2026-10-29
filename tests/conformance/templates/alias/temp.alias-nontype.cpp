//@ expect: ast
//@ oracle: clang
// [temp.alias]/2, [temp.arg.nontype]: a non-type parameter of an alias
// template substituted into an array bound.
template <class T, int N> using Arr = T[N];
Arr<int, 4> a;
int n = sizeof(Arr<char, 7>);
