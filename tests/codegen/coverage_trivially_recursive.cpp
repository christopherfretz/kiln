// Coverage of trivially_recursive.cpp at -O1 (WP-1107, spec/codegen.md
// §5.4): the zero records of the explicit-instantiation-declaration
// members ([temp.explicit]/12) come in clang's order: a specialization's
// members are deferred where their instantiation ends, never as parsed
// inline definitions.
//@ args: -O1 -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
// [temp.explicit]/12,14, [dcl.asm] (GNU asm labels): clang
// `shouldEmitFunction` does not emit the `available_externally` body of a
// function under an explicit instantiation declaration that is trivially
// recursive (`isTriviallyRecursive`): one whose body calls a function
// with its own asm label, or the `__builtin_` form of the library
// function the label names, anywhere except inside another call's
// arguments. Such a function stays a declaration; the others are emitted.
extern "C" int real_g(int) asm("gfun");
extern "C" int real_s(int) asm("sfun");
extern "C" int real_a(int) asm("afun");
extern "C" int real_g2(int) asm("gfun2");
extern "C" int real_k(int) asm("kfun");
// Calls itself through its label: a declaration only.
template <class T> inline int g(T x) asm("gfun");
template <class T> inline int g(T x) { return real_g(x); }
// The recursive call is nested in an operand: still found.
template <class T> inline int g2(T x) asm("gfun2");
template <class T> inline int g2(T x) {
  if (x) { return x > 1 ? real_g2(x) : 0; }
  return 0;
}
// Calls another label: emitted available_externally.
template <class T> inline int h(T x) asm("hfun");
template <class T> inline int h(T x) { return x ? real_g(x) + 1 : 0; }
// Calls itself as the library function `__builtin_strlen` names.
template <class T> inline unsigned long m(const T *p) asm("strlen");
template <class T> inline unsigned long m(const T *p) { return __builtin_strlen(p); }
// A recursive call inside another call's argument is not visited.
template <class T> inline int k(T x) asm("kfun");
template <class T> inline int k(T x) {
  return real_g(real_k(x));
}
// No label: never trivially recursive.
template <class T> inline int n(T x) { return n(x - 1); }
// A static member function with a label.
template <class T> struct X {
  static int s(int x) asm("sfun");
  int a(int x) asm("amem");
};
template <class T> inline int X<T>::s(int x) { return real_s(x); }
template <class T> inline int X<T>::a(int x) { return real_a(x); }
extern template int g(int);
extern template int g2(int);
extern template int h(int);
extern template unsigned long m(const char *);
extern template int k(int);
extern template int n(int);
extern template struct X<int>;
int use(X<int> &x) {
  return g(1) + g2(1) + h(2) + int(m("a")) + k(3) + n(4) + X<int>::s(5) + x.a(6);
}
