// [except.spec]/5: a friend declaration naming a constructor or
// conversion function redeclares it; written without a noexcept-specifier
// it is potentially throwing, which conflicts with a non-throwing member,
// an implicitly-declared or defaulted constructor included. A destructor
// without one has the implicit specification, so it matches.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -std=c++17
//@ strict-diags
//@ diag: 32:17 error "'Plain' is missing exception specification 'noexcept'"
//@ diag: 19:8 note "previous declaration is here"
//@ diag: 33:20 error "'Explicit' is missing exception specification 'noexcept'"
//@ diag: 21:3 note "previous declaration is here"
//@ diag: 35:20 error "'operator int' is missing exception specification 'noexcept'"
//@ diag: 23:3 note "previous declaration is here"
//@ diag: 37:21 error "'Defaulted' is missing exception specification 'noexcept'"
//@ diag: 27:3 note "previous declaration is here"

struct Plain {};
struct Explicit {
  Explicit() noexcept;
  ~Explicit();
  operator int() noexcept;
  operator long();
};
struct Defaulted {
  Defaulted() = default;
  Defaulted(const Defaulted &) noexcept(false);
};

struct Host {
  friend Plain::Plain();
  friend Explicit::Explicit();
  friend Explicit::~Explicit();
  friend Explicit::operator int();
  friend Explicit::operator long();
  friend Defaulted::Defaulted();
  friend Defaulted::Defaulted(const Defaulted &);
};
