// [class.friend]/11: an unqualified friend function of a local class
// redeclares a function of the innermost enclosing block scope, and is
// merged with it ([except.spec]/5, [over.dcl]/1 then apply). When the
// nearest block declaring the name is an outer one, or the declarations
// found do not match, clang notes each with similar parameters: "local
// declaration nearly matches", or the first parameter whose type differs
// (pointers, references and cv-qualifiers stripped, or classes of the same
// name). An operator friend finds nothing (WP-290).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:8 warning "empty parentheses interpreted as a function declaration"
//@ diag: 38:8 note "replace parentheses with an initializer to declare a variable"
//@ diag: 42:17 error "exception specification in declaration does not match previous declaration"
//@ diag: 37:8 note "previous declaration is here"
//@ diag: 43:17 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 38:7 note "previous declaration is here"
//@ diag: 44:17 error "'n' is missing exception specification 'noexcept'"
//@ diag: 39:8 note "previous declaration is here"
//@ diag: 45:17 error "no matching function found in local scope"
//@ diag: 46:14 error "no matching function found in local scope"
//@ diag: 47:17 error "no matching function found in local scope"
//@ diag: 58:19 error "no matching function found in local scope"
//@ diag: 51:8 note "local declaration nearly matches"
//@ diag: 59:19 error "no matching function found in local scope"
//@ diag: 60:19 error "no matching function found in local scope"
//@ diag: 51:10 note "type of 1st parameter of local declaration does not match definition ('int' vs 'int &')"
//@ diag: 61:19 error "no matching function found in local scope"
//@ diag: 62:19 error "no matching function found in local scope"
//@ diag: 53:16 note "type of 1st parameter of local declaration does not match definition ('const S &' vs 'T::S')"
//@ diag: 63:19 error "no matching function found in local scope"
//@ diag: 52:10 note "type of 1st parameter of local declaration does not match definition ('S *' vs 'S')"
//@ diag: 64:19 error "no matching function found in local scope"

struct S {};
namespace T { struct S {}; }
void f() {
  void g();
  int r();
  void n() noexcept;
  void m(int);
  struct L {
    friend void g() noexcept;
    friend void r();
    friend void n();
    friend void m(long);
    friend S operator+(S, S);
    friend bool operator==(L, L);
  };
}
void k() {
  void h(int);
  void p(S*, int);
  void q(const S&);
  int x;
  {
    void hx(int);
    struct M {
      friend void h(int);
      friend void hx(long);
      friend void h(int&);
      friend void p(S, long);
      friend void q(T::S);
      friend void p(S, int);
      friend void x();
    };
  }
}
void ok() {
  void g();
  void g(int);
  struct L {
    friend void g();
    friend void g(int);
  };
  g();
}
