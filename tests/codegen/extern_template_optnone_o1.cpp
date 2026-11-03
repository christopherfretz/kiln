//@ args: -O1 -Xclang -disable-llvm-passes
// [temp.explicit]/12, [dcl.inline]: `always_inline` on a member whose
// out-of-line pattern is not inline does not make its instantiation
// inline, so under an explicit instantiation declaration it stays an
// external declaration. `optnone` disables `always_inline` (clang
// -Wignored-attributes; the definition is `noinline optnone`), but an
// inline member with both is still used inline, so it is kept
// `available_externally` as every used inline member is.
template <class T> struct X {
  __attribute__((always_inline, optnone)) T f(T x) { return x; }
  __attribute__((always_inline)) T g(T x);
  __attribute__((always_inline)) T h(T x);
  T i(T x) { return x; }
};
template <class T> __attribute__((optnone)) T X<T>::g(T x) { return x; }
template <class T> inline T X<T>::h(T x) { return x; }
extern template struct X<int>;
__attribute__((always_inline, optnone)) inline int a(int x) { return x; }
int use(X<int>& p) { return a(1) + p.f(4) + p.g(5) + p.h(6) + p.i(7); }
