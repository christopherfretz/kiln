// [except.spec]/5, [temp.friend]/9, [temp.inst]/3: a non-dependent friend
// of a class template is checked against the function it redeclares when
// the template is defined; a dependent noexcept(expr) matches no other
// specification there, and a noexcept(expr) friend is checked again for
// each instantiation. A later namespace-scope declaration does not
// redeclare the pattern's friend: it is a new declaration, which each
// instantiated friend then redeclares and is checked against.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 45:15 error "'f' is missing exception specification 'noexcept'"
//@ diag: 39:6 note "previous declaration is here"
//@ diag: 46:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 40:6 note "previous declaration is here"
//@ diag: 50:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 41:6 note "previous declaration is here"
//@ diag: 51:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 42:6 note "previous declaration is here"
//@ diag: 52:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 43:6 note "previous declaration is here"
//@ diag: 47:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 60:8 note "in instantiation of template class 'A<int>' requested here"
//@ diag: 55:6 note "previous declaration is here"
//@ diag: 48:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 56:6 note "previous declaration is here"
//@ diag: 49:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 57:6 note "previous declaration is here"
//@ diag: 50:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 41:6 note "previous declaration is here"
//@ diag: 52:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 43:6 note "previous declaration is here"
//@ diag: 47:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 61:9 note "in instantiation of template class 'A<char>' requested here"
//@ diag: 55:6 note "previous declaration is here"
//@ diag: 51:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 42:6 note "previous declaration is here"
//@ diag: 52:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 43:6 note "previous declaration is here"

void f(int) noexcept;
void f2(int);
void h1(int);
void h2(int) noexcept;
void h4(int);
template <class T> struct A {
  friend void f(int);
  friend void f2(int) noexcept;
  friend void f3(int) noexcept;
  friend void f4(T) noexcept;
  friend void f5(int) noexcept(sizeof(T) > 1);
  friend void h1(int) noexcept(sizeof(T) > 1);
  friend void h2(int) noexcept(sizeof(T) > 1);
  friend void h4(int) noexcept(sizeof(int) > 1);
  friend void ok(int) noexcept;
};
void f3(int);
void f4(int);
void f5(int);
void ok(int) noexcept;

A<int> a;
A<char> c;
static_assert(noexcept(ok(1)));
