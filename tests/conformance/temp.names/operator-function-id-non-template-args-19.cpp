// [temp.names]/3, [over.oper.general]/1: operator-function-id-non-template-args-22.cpp's
// forms under clang 19, which says "member 'operator+' used before its
// declaration" for an unqualified member operator followed by template
// arguments in its class.
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 34:5 error "'operator+' does not name a template but is followed by template arguments"
//@ diag: 29:3 note "non-template declaration found by name lookup"
//@ diag: 35:6 error "'operator-' does not name a template but is followed by template arguments"
//@ diag: 31:17 note "non-template declaration found by name lookup"
//@ diag: 36:5 error "'f' does not name a template but is followed by template arguments"
//@ diag: 30:3 note "non-template declaration found by name lookup"
//@ diag: 37:6 error "'f' does not name a template but is followed by template arguments"
//@ diag: 31:36 note "non-template declaration found by name lookup"
//@ diag: 38:3 error "use of undeclared 'operator*'"
//@ diag: 39:3 error "use of undeclared 'operator*'"
//@ diag: 40:3 error "use of undeclared identifier 'zz'"
//@ diag: 47:5 error "member 'operator+' used before its declaration"
//@ diag: 44:5 note "member is declared here"
//@ diag: 48:11 error "'operator+' does not name a template but is followed by template arguments"
//@ diag: 44:5 note "non-template declaration found by name lookup"
//@ diag: 49:11 error "'m' does not name a template but is followed by template arguments"
//@ diag: 45:5 note "non-template declaration found by name lookup"
//@ diag: 54:12 error "use of undeclared 'operator\"\"_u'"

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
