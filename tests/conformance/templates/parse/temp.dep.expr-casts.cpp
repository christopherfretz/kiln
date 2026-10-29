//@ expect: ast
//@ oracle: clang
// [temp.dep.expr]/3: casts involving dependent types
template <class T> T a(int x) { return static_cast<T>(x); }
template <class T> int b(T t) { return (int)t; }
template <class T> void c(T t) { (void)t; }
template <class T> T *d(void *p) { return reinterpret_cast<T *>(p); }
