// [temp.names]/5: `nns template name` with a non-dependent `nns` names the
// member template found by qualified lookup; the `template` keyword is kept
// as written, so clang prints it in the type (`O::template J<SA>`), in the
// diagnostic `aka` (`template J<ns::S>`) and in a template template argument
// (`Q<O::template J>`, `Q< ::O::template J>`). Without the keyword a template
// template argument prints fully qualified.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:22 error "cannot initialize a variable of type 'O::template J<SA> *' (aka 'O::template J<ns::S> *') with an rvalue of type 'int'"
//@ diag: 30:13 error "cannot initialize a variable of type 'O::J<SA> *' (aka 'O::J<ns::S> *') with an rvalue of type 'int'"
//@ diag: 31:25 error "cannot initialize a variable of type 'K<O::template J<SA>> *' (aka 'K<O::template J<ns::S>> *') with an rvalue of type 'int'"
//@ diag: 32:25 error "cannot initialize a variable of type '::O::template J<int> *' with an rvalue of type 'int'"
//@ diag: 33:29 error "cannot initialize a variable of type 'const O::template J<int> *' with an rvalue of type 'int'"
//@ diag: 34:6 error "cannot initialize a variable of type 'A *' (aka 'O::template J<int> *') with an rvalue of type 'int'"
//@ diag: 35:21 error "cannot initialize a variable of type 'Q<O::template J> *' with an rvalue of type 'int'"
//@ diag: 36:23 error "cannot initialize a variable of type 'Q< ::O::template J> *' with an rvalue of type 'int'"
//@ diag: 37:12 error "cannot initialize a variable of type 'Q<O::J> *' with an rvalue of type 'int'"
//@ diag: 38:14 error "cannot initialize a variable of type 'Q<ns::T2> *' with an rvalue of type 'int'"
//@ requires: clang>=22
// clang 22 spelling of template-keyword-qualified-printing.cpp (WP-394).
namespace ns { struct S {}; template <class T> struct T2 {}; }
using SA = ns::S;
struct O { template <class T> struct J {}; };
template <class T> struct K {};
template <template <class> class TT> struct Q {};
using A = O::template J<int>;

void f() {
  O::template J<SA> *a = 1;
  O::J<SA> *b = 2;
  K<O::template J<SA>> *c = 3;
  ::O::template J<int> *d = 4;
  const O::template J<int> *e = 5;
  A *g = 6;
  Q<O::template J> *h = 7;
  Q<::O::template J> *i = 8;
  Q<O::J> *j = 9;
  Q<ns::T2> *k = 10;
}
