//@ args: -O1 -Xclang -disable-llvm-passes
// [temp.explicit]/12,14, [dcl.asm] (GNU asm labels): a call to the
// `__builtin_` form of a library function (clang `isLibFunction`, the
// `FunctionWithBuiltinPrefix` builtins, kiln `builtin::attr::LIB_FUNCTION`)
// makes a function labelled with the library name trivially recursive
// (`isTriviallyRecursive`), so its `available_externally` body is not
// emitted. A builtin that is not a library function (`__builtin_clz`,
// `__builtin_popcount`, `__builtin_trap`) never does, whatever the label.
template <class T> inline int a(T x) asm("isnan");
template <class T> inline int a(T x) { return __builtin_isnan(double(x)); }
template <class T> inline int b(T x) asm("fpclassify");
template <class T> inline int b(T x) { return __builtin_fpclassify(0, 1, 2, 3, 4, double(x)); }
template <class T> inline double c(T x) asm("powi");
template <class T> inline double c(T x) { return __builtin_powi(double(x), 2); }
template <class T> inline void d(T) asm("abort");
template <class T> inline void d(T x) { if (x) __builtin_abort(); }
template <class T> inline int e(T x) asm("clz");
template <class T> inline int e(T x) { return __builtin_clz(unsigned(x)); }
template <class T> inline int g(T x) asm("popcount");
template <class T> inline int g(T x) { return __builtin_popcount(unsigned(x)); }
template <class T> inline void h(T) asm("trap");
template <class T> inline void h(T x) { if (x) __builtin_trap(); }
extern template int a(int);
extern template int b(int);
extern template double c(int);
extern template void d(int);
extern template int e(int);
extern template int g(int);
extern template void h(int);
double use() {
  d(0);
  h(0);
  return a(1) + b(2) + c(3) + e(4) + g(5);
}
