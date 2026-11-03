// [temp.names]/3, [over.oper.general]/1: a qualified or member-access
// operator-function-id (or identifier) that names no template, followed
// by what can only be a template argument list, is "'operator+' does not
// name a template but is followed by template arguments" over the list,
// with a note at the declaration (clang's checkPotentialAngleBracket).
// Unqualified, the name is assumed a template and argument-dependent
// lookup finds none: "use of undeclared 'operator*'", or in a class "does
// not refer to a template". -19 is the clang 19 counterpart.
//@ requires: clang>=22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 38:5 error "'operator+' does not name a template but is followed by template arguments"
//@ diag: 33:3 note "non-template declaration found by name lookup"
//@ diag: 39:6 error "'operator-' does not name a template but is followed by template arguments"
//@ diag: 35:17 note "non-template declaration found by name lookup"
//@ diag: 40:5 error "'f' does not name a template but is followed by template arguments"
//@ diag: 34:3 note "non-template declaration found by name lookup"
//@ diag: 41:6 error "'f' does not name a template but is followed by template arguments"
//@ diag: 35:36 note "non-template declaration found by name lookup"
//@ diag: 42:3 error "use of undeclared 'operator*'"
//@ diag: 43:3 error "use of undeclared 'operator*'"
//@ diag: 44:3 error "use of undeclared identifier 'zz'"
//@ diag: 51:5 error "'operator+' does not refer to a template"
//@ diag: 48:5 note "declared as a non-template here"
//@ diag: 52:11 error "'operator+' does not name a template but is followed by template arguments"
//@ diag: 48:5 note "non-template declaration found by name lookup"
//@ diag: 53:11 error "'m' does not name a template but is followed by template arguments"
//@ diag: 49:5 note "non-template declaration found by name lookup"
//@ diag: 58:12 error "use of undeclared 'operator\"\"_u'"

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
