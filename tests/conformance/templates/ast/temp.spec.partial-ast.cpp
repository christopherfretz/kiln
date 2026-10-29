//@ expect: ast
//@ oracle: clang
// [temp.spec.partial]/1, [temp.spec.partial.match]/2: a class template
// partial specialization and the specialization instantiated from it.
template <class T> struct P { T v; };
template <class T> struct P<T *> { T *p; int n; };
int x = sizeof(P<int *>);
int y = sizeof(P<char>);
