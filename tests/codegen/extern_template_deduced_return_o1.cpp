//@ args: -O1 -Xclang -disable-llvm-passes
// [temp.explicit]/12, [dcl.spec.auto.general]/12: an explicit
// instantiation declaration of a function template specialization with a
// deduced return type (`extern template auto fd(int);`) still
// instantiates its definition when used, so at -O1 clang emits it
// `available_externally`, as an inline one; a non-inline one and a
// `noinline` one stay declarations. An explicit instantiation definition
// with the placeholder is `weak_odr`; an explicit specialization strong.
template <class T> auto fd(T t) { return t + 1; }
template <class T> decltype(auto) fe(T t) { return t; }
template <class T> T g(T t) { return t; }
template <class T> inline T h(T t) { return t; }
template <class T> constexpr T c(T t) { return t; }
template <class T> [[gnu::noinline]] inline T ni(T t) { return t; }
template <class T> struct S { auto n(); };
template <class T> auto S<T>::n() { return T(1); }
extern template auto fd(int);
template auto fd(long);
template <> auto fd(char x) { return x; }
extern template decltype(auto) fe(int);
extern template int g(int);
extern template int h(int);
extern template int c(int);
extern template int ni(int);
extern template auto S<int>::n();
int (*p)(short) = fd;
int use(S<int> &s) { return fd(1) + fe(2) + g(3) + h(4) + c(5) + ni(6) + s.n() + p(7); }
