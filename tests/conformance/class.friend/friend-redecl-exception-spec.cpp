// [except.spec]/5, [class.friend]: a friend declaration naming a member
// function redeclares it, so a written exception specification must match
// the member's, and one written without a specification conflicts with a
// non-throwing member (clang's out-of-line redeclaration check, applied
// to constructors, destructors, conversion functions, assignment
// operators, ordinary members and member templates). A destructor or a
// defaulted special member has an implicit specification on one side
// only: -Wimplicit-exception-spec-mismatch; a mismatch does not hide a
// constexpr mismatch (a missing specification does). A friend in a class
// template is checked too. kiln checked a friend constructor or conversion
// function written without a specification only. Same diagnostics as
// clang 22 (GCC 16.1 does not check friends).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++26
//@ xfail(gcc): GCC 16.1 does not compare a friend redeclaration's exception specification
//@ diag: 71:17 error "'f' is missing exception specification 'noexcept'"
//@ diag: 58:7 note "previous declaration is here"
//@ diag: 72:17 error "exception specification in declaration does not match previous declaration"
//@ diag: 59:7 note "previous declaration is here"
//@ diag: 77:13 error "exception specification in declaration does not match previous declaration"
//@ diag: 52:3 note "previous declaration is here"
//@ diag: 78:13 error "exception specification in declaration does not match previous declaration"
//@ diag: 53:3 note "previous declaration is here"
//@ diag: 79:13 warning "function previously declared with an explicit exception specification redeclared with an implicit exception specification [-Wimplicit-exception-spec-mismatch]"
//@ diag: 54:3 note "previous declaration is here"
//@ diag: 80:13 error "exception specification in declaration does not match previous declaration"
//@ diag: 55:3 note "previous declaration is here"
//@ diag: 81:16 error "'operator=' is missing exception specification 'noexcept'"
//@ diag: 56:6 note "previous declaration is here"
//@ diag: 82:16 error "exception specification in declaration does not match previous declaration"
//@ diag: 57:6 note "previous declaration is here"
//@ diag: 83:17 error "exception specification in declaration does not match previous declaration"
//@ diag: 58:7 note "previous declaration is here"
//@ diag: 84:17 error "exception specification in declaration does not match previous declaration"
//@ diag: 59:7 note "previous declaration is here"
//@ diag: 85:37 error "'t' is missing exception specification 'noexcept'"
//@ diag: 62:27 note "previous declaration is here"
//@ diag: 86:37 error "exception specification in declaration does not match previous declaration"
//@ diag: 63:27 note "previous declaration is here"
//@ diag: 87:13 error "exception specification in declaration does not match previous declaration"
//@ diag: 66:3 note "previous declaration is here"
//@ diag: 87:13 error "non-constexpr declaration of 'E' follows constexpr declaration"
//@ diag: 66:3 note "previous declaration is here"
//@ diag: 88:13 warning "function previously declared with an implicit exception specification redeclared with an explicit exception specification [-Wimplicit-exception-spec-mismatch]"
//@ diag: 67:3 note "previous declaration is here"
//@ diag: 88:13 error "non-constexpr declaration of '~E' follows constexpr declaration"
//@ diag: 67:3 note "previous declaration is here"

struct C {
  C() noexcept(true);
  C(int);
  ~C() noexcept(sizeof(int) > 1);
  operator bool() throw();
  C& operator=(const C&) noexcept;
  C& operator=(C&&);
  int f() noexcept;
  int g();
  int h() noexcept(sizeof(int) == 4);
  void k() throw();
  template <class T> void t(T) noexcept;
  template <class T> void u(T);
};
struct E {
  E() = default;
  ~E() = default;
};

template <class T> class InTemplate {
  friend int C::f();
  friend int C::g() noexcept;
};
InTemplate<int> it;

class Bad {
  friend C::C() noexcept(false);
  friend C::C(int) noexcept;
  friend C::~C();
  friend C::operator bool() noexcept(false);
  friend C& C::operator=(const C&);
  friend C& C::operator=(C&&) noexcept;
  friend int C::f() noexcept(false);
  friend int C::g() throw();
  template <class T> friend void C::t(T);
  template <class T> friend void C::u(T) noexcept;
  friend E::E() noexcept(false);
  friend E::~E() noexcept;
};

class Ok {
  friend C::C() noexcept;
  friend C::C(int) noexcept(false);
  friend C::~C() noexcept;
  friend C::operator bool() noexcept;
  friend C& C::operator=(const C&) noexcept(true);
  friend C& C::operator=(C&&);
  friend int C::f() throw();
  friend int C::g() noexcept(false);
  friend int C::h() noexcept;
  friend void C::k() noexcept;
  template <class T> friend void C::t(T) noexcept;
  template <class T> friend void C::u(T);
  friend constexpr E::E() noexcept;
  friend constexpr E::~E();
};
