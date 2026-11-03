// [dcl.fct]/6: a cv-qualifier-seq or ref-qualifier is only on the
// function type of a non-static member function (or the type of a
// typedef or template argument). clang reports a non-member or static
// member function's at the first qualifier written and drops them from
// the type, so a redeclaration without them is the same function.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:10 error "non-member function cannot have 'const' qualifier"
//@ diag: 22:10 error "non-member function cannot have 'volatile &&' qualifier"
//@ diag: 24:20 error "non-member function cannot have 'const' qualifier"
//@ diag: 25:20 error "non-member function cannot have '&' qualifier"
//@ diag: 26:19 error "static member function cannot have 'const &' qualifier"
//@ diag: 28:10 error "non-member function cannot have 'const' qualifier"
//@ diag: 29:24 error "non-member function cannot have '&' qualifier"
//@ diag: 30:24 error "non-member function cannot have 'volatile' qualifier"
//@ diag: 31:29 error "non-member function cannot have 'const' qualifier"
//@ diag: 32:11 error "non-member function cannot have 'const' qualifier"
//@ diag: 33:7 error "pointer to function type cannot have 'const' qualifier"
//@ diag: 34:11 error "non-member function cannot have 'const' qualifier"
//@ diag: 37:10 error "pointer to function type 'F' (aka 'void () const') cannot have 'const' qualifier"
void f() const;
void g() volatile &&;
struct B {
  friend void fr() const;
  friend void fd() & {}
  static void s() const &;
};
void h() const {}
namespace N { void k() &; }
void blk() { void bg() volatile; }
template<class T> void tf() const;
auto af() const -> int;
void (*pf)() const;
void d2() const;
void d2() {}
typedef void F() const;
void ok(F*);
