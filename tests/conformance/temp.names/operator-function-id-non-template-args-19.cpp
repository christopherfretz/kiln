// [temp.names]/3, [over.oper.general]/1: operator-function-id-non-template-args-22.cpp's
// forms under clang 19, which says "member 'operator+' used before its
// declaration" for an unqualified member operator followed by template
// arguments in its class.
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 44:5 error "'operator+' does not name a template but is followed by template arguments"
//@ diag: 39:3 note "non-template declaration found by name lookup"
//@ diag: 45:6 error "'operator-' does not name a template but is followed by template arguments"
//@ diag: 41:17 note "non-template declaration found by name lookup"
//@ diag: 46:5 error "'f' does not name a template but is followed by template arguments"
//@ diag: 40:3 note "non-template declaration found by name lookup"
//@ diag: 47:6 error "'f' does not name a template but is followed by template arguments"
//@ diag: 41:36 note "non-template declaration found by name lookup"
//@ diag: 48:3 error "use of undeclared 'operator*'"
//@ diag: 49:3 error "use of undeclared 'operator*'"
//@ diag: 50:3 error "use of undeclared identifier 'zz'"
//@ diag: 57:5 error "member 'operator+' used before its declaration"
//@ diag: 54:5 note "member is declared here"
//@ diag: 58:11 error "'operator+' does not name a template but is followed by template arguments"
//@ diag: 54:5 note "non-template declaration found by name lookup"
//@ diag: 59:11 error "'m' does not name a template but is followed by template arguments"
//@ diag: 55:5 note "non-template declaration found by name lookup"
//@ diag: 64:12 error "use of undeclared 'operator\"\"_u'"
//@ diag: 72:17 error "'f' does not name a template but is followed by template arguments"
//@ diag: 70:7 note "non-template declaration found by name lookup"
//@ diag: 72:31 error "'operator()' does not name a template but is followed by template arguments"
//@ diag: 69:7 note "non-template declaration found by name lookup"
//@ diag: 74:13 error "call to non-static member function without an object argument"
//@ diag: 74:27 error "expected '(' for function-style cast or type construction"
//@ diag: 75:13 error "call to non-static member function without an object argument"
//@ diag: 75:18 error "expected '(' for function-style cast or type construction"
//@ diag: 76:13 error "'g' does not name a template but is followed by template arguments"
//@ diag: 71:14 note "non-template declaration found by name lookup"

struct A {};
A operator+(A, A);
A f(A, A);
namespace N { A operator-(A, A); A f(A, A); }

void g(A a, A b) {
  ::operator+<int>(a, b);
  N::operator-<int>(a, b);
  ::f<int>(a, b);
  N::f<int>(a, b);
  operator*<int>(a, b);
  operator*(a, b);
  zz<int>(a);
}

struct B {
  B operator+(B);
  B m(B);
  void h(B b) {
    operator+<int>(b);
    this->operator+<int>(b);
    this->m<int>(b);
  }
};

int operator""_u(unsigned long long);
void k() { operator""_u<int>(1); }

// A non-static member named without an object is diagnosed as such; the
// `<` is then the less-than operator.
struct C {
  int operator()(int) const;
  int f(int) const;
  static int g(int);
  void m() { C::f<int>(1); C::operator()<int>(1); }
};
int c1 = C::operator()<int>(1);
int c2 = C::f<int>(1);
int c3 = C::g<int>(1);
