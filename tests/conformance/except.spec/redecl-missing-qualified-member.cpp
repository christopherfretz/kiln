// [except.spec]/5: an out-of-line definition of a member function with a
// cv- or ref-qualifier that omits the previous `noexcept` is diagnosed at
// its name; clang's fix-it inserts the specification after the last
// qualifier (checked against clang's full output in kiln_driver's
// `exception_spec_fixits_and_ranges_like_clang`; WP-279).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:9 error "'f' is missing exception specification 'noexcept'"
//@ diag: 17:8 note "previous declaration is here"
//@ diag: 23:9 error "'g' is missing exception specification 'noexcept'"
//@ diag: 18:8 note "previous declaration is here"
//@ diag: 24:9 error "'h' is missing exception specification 'noexcept'"
//@ diag: 19:8 note "previous declaration is here"
//@ diag: 25:9 error "'i' is missing exception specification 'noexcept'"
//@ diag: 20:8 note "previous declaration is here"
struct A {
  void f() const noexcept;
  void g() & noexcept;
  void h() const volatile && noexcept;
  void i() volatile const noexcept;
};
void A::f() const {}
void A::g() & {}
void A::h() const volatile && {}
void A::i() volatile const {}
static_assert(noexcept(A().h()) && noexcept(static_cast<A &&>(A()).h()));
