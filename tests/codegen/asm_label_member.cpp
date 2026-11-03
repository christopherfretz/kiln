// [dcl.asm] (GNU asm labels), [temp.inst]: a member function's asm label
// replaces its linkage name as a namespace-scope function's does (clang
// `ActOnFunctionDeclarator`), and an instantiated function or member has
// its pattern's label (clang `InstantiateAttrs`).
struct S { int g(int) asm("sg"); };
int S::g(int x) { return x; }
template <class T> int t(T) asm("tl");
template <class T> int t(T x) { return x; }
template <class T> struct X { int g(int) asm("xg"); };
template <class T> int X<T>::g(int x) { return x; }
int use(S &s, X<int> &x) { return s.g(1) + t(2) + x.g(3); }
