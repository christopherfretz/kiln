// [temp.expl.spec] [clang ext] An explicit specialization of a function
// template, or of a member function of a class template, has the
// template's (member's) calling convention, written again or not, and an
// explicit instantiation keeps it: each lowers with it on its definition
// and at every call site, direct and through a pointer (clang
// `adjustCCAndNoReturn`). Reduced from real code. WP-1843.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
#define PM __attribute__((preserve_most))
#define PA __attribute__((preserve_all))
template <int N> PM int g(int x) { return x + N; }
template <> PM int g<2>(int x) { return x * 2; }
template <> int g<3>(int x) { return x * 3; }
template int g<4>(int) PA;
template <class T> __attribute__((swiftcall)) T s(T x) { return x; }
template <> int s(int x) { return x + 1; }
template <class T> struct A {
  PA int f(int);
  template <class U> PM static U h(U u) { return u; }
};
template <> int A<int>::f(int x) { return x - 1; }
template <> template <> long A<int>::h(long u) { return u + 1; }
using PMF = PM int (*)(int);
PMF table[] = {&g<1>, &g<2>, &g<3>};
int use(A<int> &a) {
  return g<1>(1) + g<2>(2) + g<3>(3) + s(4) + a.f(5) + int(A<int>::h(6L)) + table[1](7);
}
