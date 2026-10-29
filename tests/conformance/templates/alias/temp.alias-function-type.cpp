//@ expect: ast
//@ oracle: clang
// [temp.alias]/2, [dcl.fct]: an alias template naming a function pointer type.
template <class R, class A> using Fn = R (*)(A);
int twice(int x) { return 2 * x; }
Fn<int, int> f = twice;
