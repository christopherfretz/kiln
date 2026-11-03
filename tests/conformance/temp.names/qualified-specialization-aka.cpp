// [temp.names]/1, [basic.lookup.qual]: a qualified simple-template-id names
// the specialization; the diagnostic `aka` desugars the written type the way
// clang's desugarForDiagnostic does, so a specialization prints its template
// name unqualified with desugared arguments (`'O::J<SA>' (aka 'J<ns::S>')`,
// `'KA' (aka 'K<ns::S>')`), while a plain record argument prints fully scoped
// (`ns::S`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 70:12 error "no viable conversion from 'int' to 'O::J<SA>' (aka 'J<ns::S>')"
//@ diag: 60:38 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const J<S> &' for 1st argument"
//@ diag: 60:38 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'J<S> &&' for 1st argument"
//@ diag: 71:16 error "no viable conversion from 'int' to 'ns::O::J<SA>' (aka 'J<ns::S>')"
//@ diag: 60:38 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const J<S> &' for 1st argument"
//@ diag: 60:38 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'J<S> &&' for 1st argument"
//@ diag: 72:13 error "no viable conversion from 'int' to 'ns::K<SA>' (aka 'K<ns::S>')"
//@ diag: 61:27 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const K<S> &' for 1st argument"
//@ diag: 61:27 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'K<S> &&' for 1st argument"
//@ diag: 73:17 error "no viable conversion from 'int' to 'ns::in::L<SA>' (aka 'L<ns::S>')"
//@ diag: 63:42 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const L<S> &' for 1st argument"
//@ diag: 63:42 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'L<S> &&' for 1st argument"
//@ diag: 74:15 error "no viable conversion from 'int' to '::ns::K<SA>' (aka 'K<ns::S>')"
//@ diag: 61:27 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const K<S> &' for 1st argument"
//@ diag: 61:27 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'K<S> &&' for 1st argument"
//@ diag: 75:19 error "cannot initialize a variable of type 'const O::J<SA> *' (aka 'const J<ns::S> *') with an rvalue of type 'int'"
//@ diag: 76:17 error "non-const lvalue reference to type 'ns::O::J<SA>' (aka 'J<ns::S>') cannot bind to a temporary of type 'int'"
//@ diag: 77:20 error "no viable conversion from 'int' to 'ns::K<ns::K<SA>>' (aka 'K<K<ns::S>>')"
//@ diag: 61:27 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const K<K<S>> &' for 1st argument"
//@ diag: 61:27 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'K<K<S>> &&' for 1st argument"
//@ diag: 78:21 error "no viable conversion from 'int' to 'ns::K2<ns::S, SA>' (aka 'K2<ns::S, ns::S>')"
//@ diag: 62:36 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const K2<S, S> &' for 1st argument"
//@ diag: 62:36 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'K2<S, S> &&' for 1st argument"
//@ diag: 79:13 error "no viable conversion from 'int' to 'ns::K<KA>' (aka 'K<K<ns::S>>')"
//@ diag: 61:27 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const K<K<S>> &' for 1st argument"
//@ diag: 61:27 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'K<K<S>> &&' for 1st argument"
//@ diag: 80:6 error "no viable conversion from 'int' to 'KA' (aka 'K<ns::S>')"
//@ diag: 61:27 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const K<S> &' for 1st argument"
//@ diag: 61:27 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'K<S> &&' for 1st argument"
//@ diag: 81:15 error "no viable conversion from 'int' to 'ns::K<SA *>' (aka 'K<ns::S *>')"
//@ diag: 61:27 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const K<S *> &' for 1st argument"
//@ diag: 61:27 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'K<S *> &&' for 1st argument"
//@ diag: 82:19 error "no viable conversion from 'int' to 'ns::K<void (SA)>' (aka 'K<void (ns::S)>')"
//@ diag: 61:27 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const K<void (S)> &' for 1st argument"
//@ diag: 61:27 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'K<void (S)> &&' for 1st argument"
//@ diag: 83:10 error "no viable conversion from 'int' to 'Al<SA>' (aka 'K<ns::S>')"
//@ diag: 61:27 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const K<S> &' for 1st argument"
//@ diag: 61:27 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'K<S> &&' for 1st argument"
//@ diag: 84:23 error "no viable conversion from 'int' to 'ns::K<ns::O::J<SA>>' (aka 'K<J<ns::S>>')"
//@ diag: 61:27 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const K<J<S>> &' for 1st argument"
//@ diag: 61:27 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'K<J<S>> &&' for 1st argument"
//@ diag: 85:13 error "no viable conversion from 'int' to 'O::J<int>'"
//@ diag: 60:38 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const J<int> &' for 1st argument"
//@ diag: 60:38 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'J<int> &&' for 1st argument"
//@ diag: 87:15 error "no viable conversion from 'int' to 'decltype(s)' (aka 'K<ns::S>')"
//@ diag: 61:27 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const K<S> &' for 1st argument"
//@ diag: 61:27 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'K<S> &&' for 1st argument"
//@ requires: clang<22
// clang 19 spelling; qualified-specialization-aka-clang22.cpp has clang 22's (WP-394).
namespace ns {
struct S {};
struct O { template <class T> struct J {}; };
template <class T> struct K {};
template <class T, class U> struct K2 {};
namespace in { template <class T> struct L {}; }
}
using SA = ns::S;
using O = ns::O;
using KA = ns::K<SA>;
template <class T> using Al = ns::K<T>;
void f() {
  O::J<SA> a = 1;
  ns::O::J<SA> b = 1;
  ns::K<SA> c = 1;
  ns::in::L<SA> d = 1;
  ::ns::K<SA> e = 1;
  const O::J<SA> *g = 1;
  ns::O::J<SA> &h = 1;
  ns::K<ns::K<SA>> i = 1;
  ns::K2<ns::S, SA> j = 1;
  ns::K<KA> k = 1;
  KA l = 1;
  ns::K<SA *> m = 1;
  ns::K<void(SA)> n = 1;
  Al<SA> o = 1;
  ns::K<ns::O::J<SA>> p = 1;
  O::J<int> q = 1;
  ns::K<SA> s;
  decltype(s) r = 1;
}
