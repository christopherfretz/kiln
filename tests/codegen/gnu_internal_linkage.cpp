// clang `internal_linkage` (WP-807): functions, variables, members, classes
// (their members, vtables and type info), function / variable / class
// templates get internal linkage; at namespace scope the name mangles with
// `L`, as a `static` function template's does.
__attribute__((internal_linkage)) void f() {}
[[clang::internal_linkage]] int v = 1;
struct S {
  __attribute__((internal_linkage)) void m() {}
  __attribute__((internal_linkage)) static int sv;
  __attribute__((internal_linkage)) static void sm();
};
int S::sv = 2;
void S::sm() {}
class __attribute__((internal_linkage)) C { public: void cm() {} static int cv; };
int C::cv = 3;
inline __attribute__((internal_linkage)) void inl() {}
template <class T> __attribute__((internal_linkage)) void tf() {}
template <class T> static void st() {}
namespace N { __attribute__((internal_linkage)) int nv = 4; }
template <class T> __attribute__((internal_linkage)) T vt = T(7);
template <class T> struct __attribute__((internal_linkage)) CT { static T sv; T get() { return sv; } };
template <class T> T CT<T>::sv = T(5);
struct __attribute__((internal_linkage)) V { virtual int f() { return 1; } virtual ~V() {} };
namespace N { struct __attribute__((internal_linkage)) W { int g(); }; int W::g() { return 2; } }
template <class T> struct Box { T t; int h() { return 3; } };
__attribute__((internal_linkage)) int addr_taken() { return 4; }
int (*fp)() = addr_taken;
void use1(S &s, C &c) {
  f(); s.m(); S::sm(); (void)v; (void)S::sv; c.cm(); (void)C::cv; inl(); tf<int>(); st<int>();
  (void)N::nv;
}
int use2(int i) {
  V v; Box<N::W> b; CT<int> c; CT<long> c2;
  return vt<int> + vt<long> + c.get() + (int)c2.get() + v.f() + b.h() + b.t.g() + fp();
}
