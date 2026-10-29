//@ expect: ast
//@ oracle: clang
// [temp.param]/10, [temp.alias]: default template arguments of an alias
// template.
template <class T, class U = T *> using Second = U;
Second<int> p;
Second<int, long> l;
