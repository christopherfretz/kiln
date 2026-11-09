// [temp.local]/6: a template parameter cannot be redeclared within its
// scope: by an alias-declaration or alias template, a nested template
// head's parameter, a function or handler parameter, a namespace alias,
// or a friend class template (clang DiagnoseTemplateParameterShadow;
// WP-1042).
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ diag: 27:20 error "declaration of 'T' shadows template parameter"
//@ diag: 26:16 note "template parameter is declared here"
//@ diag: 28:18 error "declaration of 'T' shadows template parameter"
//@ diag: 26:16 note "template parameter is declared here"
//@ diag: 29:14 error "declaration of 'T' shadows template parameter"
//@ diag: 26:16 note "template parameter is declared here"
//@ diag: 30:27 error "declaration of 'T' shadows template parameter"
//@ diag: 26:16 note "template parameter is declared here"
//@ diag: 32:48 error "declaration of 'T' shadows template parameter"
//@ diag: 32:16 note "template parameter is declared here"
//@ diag: 33:40 error "declaration of 'T' shadows template parameter"
//@ diag: 33:16 note "template parameter is declared here"
//@ diag: 35:42 error "declaration of 'V' shadows template parameter"
//@ diag: 34:14 note "template parameter is declared here"
//@ diag: 37:36 error "declaration of 'T' shadows template parameter"
//@ diag: 37:16 note "template parameter is declared here"
namespace N {}
template<class T> struct A {
  struct F { using T = int; };
  template<class T> struct B {};
  void f(int T);
  template<class U> using T = int;
};
template<class T> void g() { try {} catch (int T) {} }
template<class T> void h() { namespace T = N; }
template<int V> struct C {
  struct D { template<int> friend struct V; };
};
template<class T> void k() { using T = int; }
