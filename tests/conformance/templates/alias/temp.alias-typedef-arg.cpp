//@ expect: ast
//@ oracle: clang
// [temp.alias]/2: the alias names the substituted type; the argument is
// written through a typedef.
typedef int I;
template <class T> using Ptr = T *;
Ptr<I> p;
Ptr<const I> cp;
