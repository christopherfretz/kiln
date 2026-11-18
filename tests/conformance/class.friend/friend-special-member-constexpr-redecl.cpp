// [dcl.constexpr]/1: every declaration of a constexpr or consteval
// function has the same specifier. A friend declaration naming a
// constructor or destructor redeclares it, an implicitly-declared or
// defaulted constexpr one included: kiln compared ordinary functions and
// members only. Same errors as clang 22 (GCC 16.1 does not check
// friends); a missing exception specification is reported instead, as
// clang does. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ xfail(gcc): GCC 16.1 does not compare a friend redeclaration's constexpr specifier
//@ diag: 45:13 error "non-constexpr declaration of 'B' follows constexpr declaration"
//@ diag: 29:8 note "previous declaration is here"
//@ diag: 46:13 error "non-constexpr declaration of 'B' follows constexpr declaration"
//@ diag: 29:8 note "previous declaration is here"
//@ diag: 47:13 error "non-constexpr declaration of '~B' follows constexpr declaration"
//@ diag: 29:8 note "previous declaration is here"
//@ diag: 48:14 error "non-constexpr declaration of 'B2' follows constexpr declaration"
//@ diag: 31:13 note "previous declaration is here"
//@ diag: 49:24 error "constexpr declaration of 'B2' follows non-constexpr declaration"
//@ diag: 32:3 note "previous declaration is here"
//@ diag: 50:14 error "non-constexpr declaration of 'B4' follows constexpr declaration"
//@ diag: 35:3 note "previous declaration is here"
//@ diag: 53:13 error "'B' is missing exception specification 'noexcept'"
//@ diag: 29:8 note "previous declaration is here"
//@ diag: 57:14 error "non-constexpr declaration of 'B2' follows constexpr declaration"
//@ diag: 31:13 note "previous declaration is here"

struct B {};
struct B2 {
  constexpr B2() {}
  B2(const B2 &);
};
struct B4 {
  B4() = default;
};
struct NL {
  NL();
};
struct B5 {
  NL n;
};

struct A {
  friend B::B() noexcept;
  friend B::B(const B &) noexcept;
  friend B::~B();
  friend B2::B2();
  friend constexpr B2::B2(const B2 &);
  friend B4::B4() noexcept;
  friend B5::B5();          // not constexpr: fine
  friend constexpr B::B(B &&) noexcept; // fine
  friend B::B(B &&);        // the exception specification is reported
};

template <class T> struct Y {
  friend B2::B2();
};
