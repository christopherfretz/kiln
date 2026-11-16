// [dcl.type.elab]/1: `class-key nested-name-specifier template
// simple-template-id`, after a class, a namespace and a dependent
// qualifier, in a declaration, a pointer declarator and a friend
// declaration; `typename T::template X<int>::type` stays as it was
// (WP-1495; found running GCC's C++ testsuite).
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
template<typename T> struct A { template<typename U> struct W {}; };
namespace N { template<class T> struct S {}; }
template<class T> void h() {
  typename T::template X<int>::type *p = nullptr;
  struct T::template X<int> *q = nullptr;
  (void)p; (void)q;
}
struct Q { template<class> struct X { typedef int type; }; };
void g() {
  struct A<int>::template W<int> w;
  struct N::template S<int> s;
  class A<char>::template W<long> *pw = nullptr;
  (void)w; (void)s; (void)pw;
  h<Q>();
}
