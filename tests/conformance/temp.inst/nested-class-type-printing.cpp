// Diagnostic type strings ([temp.inst], [class.nest]): a type substituted
// for a template parameter is canonical, so a nested class or nested class
// template specialization prints qualified by its enclosing class
// ('view<int>::iterator<false>'), with no aka. A type written by its
// unqualified name inside its class or namespace prints as written, also
// with no aka: only sugar such as a typedef or decltype asks for one
// (clang's desugarForDiagnostic).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 48:7 error "no viable conversion from 'S' to 'int'"
//@ diag: 49:7 error "no viable conversion from 'I' to 'int'"
//@ diag: 50:7 error "no viable conversion from 'J<int>' to 'int'"
//@ diag: 52:7 error "no viable conversion from 'iterator<false>' to 'int'"
//@ diag: 53:7 error "no viable conversion from 'plain' to 'int'"
//@ diag: 55:7 error "no viable conversion from 'iterator<false>' to 'int'"
//@ diag: 57:7 error "no viable conversion from 'decltype(v.p())' (aka 'view<int>::plain') to 'int'"
//@ diag: 58:7 error "cannot initialize a variable of type 'int' with an rvalue of type 'SA *' (aka 'ns::S *')"
//@ diag: 42:41 error "no viable conversion from 'view<int>::iterator<false>' to 'int'"
//@ diag: 59:3 note "in instantiation of function template specialization 'take<view<int>::iterator<false>>' requested here"
//@ diag: 42:41 error "no viable conversion from 'view<int>::plain' to 'int'"
//@ diag: 60:3 note "in instantiation of function template specialization 'take<view<int>::plain>' requested here"
//@ diag: 45:18 error "no viable conversion from 'view<int>::plain' to 'int'"
//@ diag: 61:25 note "in instantiation of member function 'W<view<int>::plain>::h' requested here"
namespace ns {
struct S {};
S mk();
struct O {
  struct I {};
  I g();
  template <class U> struct J {};
  J<int> h();
};
using SA = S;
SA *mkp();
}
template <class T> struct view {
  template <bool C> struct iterator {};
  struct plain {};
  iterator<false> begin();
  plain p();
};
template <class T> void take(T t) { int x = t; }
template <class T> struct W {
  T m;
  void h() { int y = m; }
};
void f() {
  int a = ns::mk();
  int b = ns::O().g();
  int c = ns::O().h();
  view<int> v;
  int d = v.begin();
  int e = v.p();
  auto z = v.begin();
  int g = z;
  decltype(v.p()) y;
  int h = y;
  int i = ns::mkp();
  take(v.begin());
  take(v.p());
  W<view<int>::plain>().h();
}
