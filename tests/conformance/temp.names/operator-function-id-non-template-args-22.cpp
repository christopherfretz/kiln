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
//@ diag: 48:5 error "'operator+' does not name a template but is followed by template arguments"
//@ diag: 43:3 note "non-template declaration found by name lookup"
//@ diag: 49:6 error "'operator-' does not name a template but is followed by template arguments"
//@ diag: 45:17 note "non-template declaration found by name lookup"
//@ diag: 50:5 error "'f' does not name a template but is followed by template arguments"
//@ diag: 44:3 note "non-template declaration found by name lookup"
//@ diag: 51:6 error "'f' does not name a template but is followed by template arguments"
//@ diag: 45:36 note "non-template declaration found by name lookup"
//@ diag: 52:3 error "use of undeclared 'operator*'"
//@ diag: 53:3 error "use of undeclared 'operator*'"
//@ diag: 54:3 error "use of undeclared identifier 'zz'"
//@ diag: 61:5 error "'operator+' does not refer to a template"
//@ diag: 58:5 note "declared as a non-template here"
//@ diag: 62:11 error "'operator+' does not name a template but is followed by template arguments"
//@ diag: 58:5 note "non-template declaration found by name lookup"
//@ diag: 63:11 error "'m' does not name a template but is followed by template arguments"
//@ diag: 59:5 note "non-template declaration found by name lookup"
//@ diag: 68:12 error "use of undeclared 'operator\"\"_u'"
//@ diag: 76:17 error "'f' does not name a template but is followed by template arguments"
//@ diag: 74:7 note "non-template declaration found by name lookup"
//@ diag: 76:31 error "'operator()' does not name a template but is followed by template arguments"
//@ diag: 73:7 note "non-template declaration found by name lookup"
//@ diag: 78:13 error "call to non-static member function without an object argument"
//@ diag: 78:27 error "expected '(' for function-style cast or type construction"
//@ diag: 79:13 error "call to non-static member function without an object argument"
//@ diag: 79:18 error "expected '(' for function-style cast or type construction"
//@ diag: 80:13 error "'g' does not name a template but is followed by template arguments"
//@ diag: 75:14 note "non-template declaration found by name lookup"

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
