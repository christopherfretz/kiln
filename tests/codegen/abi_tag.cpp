// abi_tag mangling (WP-172): <abi-tags> after the unqualified name of
// functions, variables, structors and operators; instantiated tags come
// from the pattern definition (clang 19), so an out-of-line member
// definition of a class template that does not repeat the tag drops it,
// unless the class specialization was instantiated before that definition
// was seen (the member's attributes are then taken from the in-class
// declaration).
int v __attribute__((abi_tag("x"))) = 1;
namespace N { int w __attribute__((abi_tag("b", "a", "b"))) = 2; }
__attribute__((abi_tag("t1"))) int f(int x) { return x + v; }
struct S {
  __attribute__((abi_tag("ctor"))) S() {}
  __attribute__((abi_tag("dt"))) ~S() {}
  __attribute__((abi_tag("op"))) bool operator==(const S&) const { return true; }
  __attribute__((abi_tag("m"))) int get() const { return 3; }
  template <class T> __attribute__((abi_tag("tm"))) T conv(T t) const { return t; }
};
template <class T> struct W {
  __attribute__((abi_tag("wt"))) T get() const { return T(); }
  int plain();
};
template <class T> int W<T>::plain() { return get(); }
template <class T> __attribute__((abi_tag("ft"))) T id(T t) { return t; }
int g() {
  S s, t;
  W<int> w;
  return f(1) + (s == t) + s.get() + s.conv(4) + w.plain() + id(5) + N::w;
}
template <class T> struct W4 {
  template <class U> __attribute__((abi_tag("ct"))) W4(U) {}
  template <class U> __attribute__((abi_tag("mt"))) void m(U) {}
};
template <class T> struct X4 {
  template <class U> __attribute__((abi_tag("xo"))) X4(U);
};
template <class T> template <class U> X4<T>::X4(U) {}
void g4() { W4<int> w(1.0); w.m(2); X4<int> x(3); }
template <class T> struct Y5 { __attribute__((abi_tag("y"))) int f(); template <class U> __attribute__((abi_tag("z"))) int g(U); };
template <class T> int Y5<T>::f() { return 0; }
template <class T> template <class U> int Y5<T>::g(U) { return 0; }
struct Z5 { template <class U> __attribute__((abi_tag("q"))) int h(U); };
template <class U> int Z5::h(U) { return 0; }
__attribute__((abi_tag("r"))) int k5();
int k5() { return 1; }
int u5() { Y5<int> y; Z5 z; return y.f() + y.g(1) + z.h(1) + k5(); }
template <class T> struct V6 { __attribute__((abi_tag("v"))) int f(); template <class U> __attribute__((abi_tag("w"))) int g(U); static int s __attribute__((abi_tag("s"))); };
template <class T> int V6<T>::s = 3;
template <class T> struct Q6 { __attribute__((abi_tag("v"))) int f(); };
template <class T> __attribute__((abi_tag("v"))) int Q6<T>::f() { return 0; }
int u6() { V6<int> v; Q6<int> q; return v.f() + v.g(1) + V6<int>::s + q.f(); }
template <class T> struct A7 { __attribute__((abi_tag("a"))) int f(); template <class U> __attribute__((abi_tag("ag"))) int g(U); };
inline int sizeA7 = sizeof(A7<int>);
template <class T> int A7<T>::f() { return 0; }
template <class T> template <class U> int A7<T>::g(U) { return 0; }
template <class T> struct B7 { __attribute__((abi_tag("b"))) int f(); };
extern template struct B7<int>;
template <class T> int B7<T>::f() { return 0; }
int u7() { return A7<int>().f() + A7<int>().g(1) + B7<int>().f(); }
