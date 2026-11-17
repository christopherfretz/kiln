// [class.access]/4: a member class or alias template named as a template
// template argument, or as a template template parameter's default, is
// access-checked where it is written.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:16 error "'Inner' is a private member of 'Hidden'"
//@ diag: 15:27 note "implicitly declared private here"
//@ diag: 26:16 error "'Alias' is a private member of 'Hidden'"
//@ diag: 16:20 note "implicitly declared private here"
//@ diag: 29:46 error "'Inner' is a private member of 'Hidden'"
//@ diag: 15:27 note "implicitly declared private here"

class Hidden {
  template <class> struct Inner;
  template <class> using Alias = int;
  friend struct Friend;

public:
  template <class> struct Open;
};

template <template <class> class T> struct Holder {};

Holder<Hidden::Inner> h1;
Holder<Hidden::Alias> h2;
Holder<Hidden::Open> h3;

template <template <class> class T = Hidden::Inner> struct WithDefault;

struct Friend {
  Holder<Hidden::Inner> ok;
};

struct Derived : Hidden {
  Holder<Open> ok;
};
