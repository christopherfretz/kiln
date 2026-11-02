// [temp.names]/3 (P0846): from C++20 an unqualified name or
// operator-function-id followed by `<` is assumed to name a template when
// lookup finds only functions, members included. When no function
// template is found, clang 22 diagnoses "'f' does not refer to a
// template" (with the first declaration found) lexically inside a class
// -- a member function body, through a lambda, a default member
// initializer, an out-of-line member, a class template's member -- and
// an undeclared name elsewhere. A class member found by lookup suppresses
// argument-dependent lookup ([basic.lookup.argdep]/3). clang 19 instead
// says "member 'operator()' used before its declaration".
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ requires: clang>=22
//@ diag: 36:20 error "'operator()' does not refer to a template"
//@ diag: 35:7 note "declared as a non-template here"
//@ diag: 40:20 error "'operator()' does not refer to a template"
//@ diag: 39:7 note "declared as a non-template here"
//@ diag: 44:36 error "'operator()' does not refer to a template"
//@ diag: 43:7 note "declared as a non-template here"
//@ diag: 48:11 error "'operator()' does not refer to a template"
//@ diag: 47:7 note "declared as a non-template here"
//@ diag: 54:21 error "'operator()' does not refer to a template"
//@ diag: 51:7 note "declared as a non-template here"
//@ diag: 60:28 error "'operator+' does not refer to a template"
//@ diag: 59:7 note "declared as a non-template here"
//@ diag: 62:27 error "use of undeclared 'operator+'"
//@ diag: 66:20 error "'k' does not refer to a template"
//@ diag: 64:5 note "declared as a non-template here"
//@ diag: 68:20 error "'m' does not refer to a template"
//@ diag: 67:7 note "declared as a non-template here"
//@ diag: 70:23 error "use of undeclared identifier 'k'"
//@ diag: 72:29 error "use of undeclared identifier 'u'"
struct N {
  int operator()(int x) { return x; }
  int g() { return operator()<int>(1); }
};
template <class X> struct D {
  int operator()(int x) { return x; }
  int g() { return operator()<int>(1); }
};
struct L {
  int operator()(int x) { return x; }
  int g() { return [this] { return operator()<int>(1); }(); }
};
struct P {
  int operator()(int x) { return x; }
  int k = operator()<int>(1);
};
struct Q {
  int operator()(int);
  int g();
};
int Q::g() { return operator()<int>(1); }

struct A { int v; };
int operator+(A a, A b) { return a.v + b.v; }
struct O {
  int operator+(int x) { return x; }
  int g() { A a{1}; return operator+<int>(a, a); }
};
int free_op(A a) { return operator+<int>(a, a); }

int k(int);
struct S {
  int g() { return k<int>(1); }
  int m(int);
  int h() { return m<int>(1); }
};
int free_k() { return k<int>(1); }
namespace ns { struct B {}; int u(B); }
struct R { int g() { return u<int>(ns::B{}); } };
