// [temp.param]/12: no default template argument on a friend template
// declaration that is not a function template definition: a friend
// function template declaration (each defaulted parameter, named or not),
// a friend class template (unqualified or qualified), and a friend function
// template declared in a class template. Valid pair:
// friend-template-default-arg-valid.cpp (WP-1015).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 21:17 error "default template argument not permitted on a friend template"
//@ diag: 21:30 error "default template argument not permitted on a friend template"
//@ diag: 22:19 error "default template argument not permitted on a friend template"
//@ diag: 22:30 error "default template argument not permitted on a friend template"
//@ diag: 23:36 error "default template argument not permitted on a friend template"
//@ diag: 24:19 error "default template argument not permitted on a friend template"
//@ diag: 25:19 error "default template argument not permitted on a friend template"
//@ diag: 28:19 error "default template argument not permitted on a friend template"
//@ diag: 32:49 error "default template argument not permitted on a friend template"
namespace N { template <class T> struct Q; }
template <class T> struct R;
struct S {
  template <int N = 1, class U = int> friend void h(U);
  template <class = int, int = 2> friend void h2();
  template <template <class> class TT = R> friend void h3();
  template <class T = int> friend struct R;
  template <class T = int> friend struct N::Q;
};
template <class T> struct X {
  template <class U = T> friend void p();
};
X<int> xi;
template <class T> struct Y {
  template <class U> struct Z { template <class V = U> friend void r(); };
};
