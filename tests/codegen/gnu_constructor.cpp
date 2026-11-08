// GNU `constructor` / `destructor` (WP-807): `@llvm.global_ctors` /
// `@llvm.global_dtors` entries in emission order, after the unordered
// initializers and before `_GLOBAL__sub_I_<file>`; static, inline and
// in-class definitions are emitted unused; the definition's own priority
// wins over an earlier declaration's; template instantiations get theirs.
int side(int);
int glob = side(1);
template <class T> struct W { static int m; };
template <class T> int W<T>::m = side(2);
template struct W<int>;
__attribute__((constructor, destructor)) void both() {}
__attribute__((constructor(200), constructor(300))) void twice() {}
__attribute__((constructor(200))) void redecl();
__attribute__((constructor(300))) void redecl() {}
static __attribute__((constructor)) void st() {}
inline __attribute__((constructor)) void in() {}
__attribute__((constructor)) void ext();
__attribute__((constructor(-1))) int nonvoid(int) { return 0; }
struct S {
  __attribute__((constructor(120))) static void sm() {}
  __attribute__((destructor(130))) void m() {}
};
template <class T> __attribute__((constructor)) void t() {}
template <int N> struct X { __attribute__((destructor(150))) static void f() {} };
template void t<int>();
template struct X<1>;
void use() { X<2>::f(); ext(); }
