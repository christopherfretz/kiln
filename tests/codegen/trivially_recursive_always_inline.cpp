// [temp.explicit]/12,14, [dcl.asm]: at -O0 an `always_inline` function
// under an explicit instantiation declaration is emitted
// `available_externally` for the always-inliner unless it is trivially
// recursive (clang `shouldEmitFunction`, `isTriviallyRecursive`): `a`
// calls its own label and stays a declaration, `b` is inlined.
extern "C" int real_a(int) asm("afun");
template <class T> inline __attribute__((always_inline)) int a(T x) asm("afun");
template <class T> inline int a(T x) { return real_a(x); }
template <class T> inline __attribute__((always_inline)) int b(T x) { return real_a(x); }
extern template int a(int);
extern template int b(int);
int use() { return a(1) + b(2); }
