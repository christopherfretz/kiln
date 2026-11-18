// [class.friend]/7, [basic.lookup.qual]/1 and [except.spec]/5: a friend
// declaration redeclaring a member function is not found by lookup in
// the member's class, so a later redeclaration (another friend, an
// out-of-line definition) is checked against the member: "previous
// declaration" is the member and a missing exception specification is
// spelled as the member's (clang 22). A friend's specification must
// match the member's, so which one is compared cannot change the
// verdict, only the location and spelling. GCC 16.1 accepts the missing
// specifications and rejects the different one.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++14
//@ diag: 35:8 error "'f' is missing exception specification 'noexcept'"
//@ diag: 24:7 note "previous declaration is here"
//@ diag: 36:8 error "'g' is missing exception specification 'throw()'"
//@ diag: 25:7 note "previous declaration is here"
//@ diag: 38:17 error "'h' is missing exception specification 'noexcept'"
//@ diag: 26:7 note "previous declaration is here"
//@ diag: 40:8 error "exception specification in declaration does not match previous declaration"
//@ diag: 27:7 note "previous declaration is here"

struct C {
  int f() noexcept;
  int g() throw();
  int h() noexcept;
  int k() noexcept;
};
class A {
  friend int C::f() throw();
  friend int C::g() noexcept;
  friend int C::h() throw();
  friend int C::k() throw();
};
int C::f() { return 0; }
int C::g() { return 0; }
class B {
  friend int C::h();
};
int C::k() noexcept(false) { return 0; }
