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
//@ diag: 40:8 warning "empty parentheses interpreted as a function declaration"
//@ diag: 40:8 note "replace parentheses with an initializer to declare a variable"
//@ diag: 44:17 error "exception specification in declaration does not match previous declaration"
//@ diag: 39:8 note "previous declaration is here"
//@ diag: 45:17 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 40:7 note "previous declaration is here"
//@ diag: 46:17 error "'n' is missing exception specification 'noexcept'"
//@ diag: 41:8 note "previous declaration is here"
//@ diag: 47:17 error "cannot define friend function in a local class definition"
//@ diag: 48:14 error "cannot define friend function in a local class definition"
//@ diag: 49:17 error "cannot define friend function in a local class definition"
//@ diag: 60:19 error "cannot define friend function in a local class definition"
//@ diag: 53:8 note "local declaration nearly matches"
//@ diag: 61:19 error "cannot define friend function in a local class definition"
//@ diag: 62:19 error "cannot define friend function in a local class definition"
//@ diag: 53:10 note "type of 1st parameter of local declaration does not match definition ('int' vs 'int &')"
//@ diag: 63:19 error "cannot define friend function in a local class definition"
//@ diag: 64:19 error "cannot define friend function in a local class definition"
//@ diag: 55:16 note "type of 1st parameter of local declaration does not match definition ('const S &' vs 'T::S')"
//@ diag: 65:19 error "cannot define friend function in a local class definition"
//@ diag: 54:10 note "type of 1st parameter of local declaration does not match definition ('S *' vs 'S')"
//@ diag: 66:19 error "cannot define friend function in a local class definition"
//@ requires: clang>=22
// kiln at -fclang-compat=22 (WP-377); clang 19 wording: "no matching function found in local scope"

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
