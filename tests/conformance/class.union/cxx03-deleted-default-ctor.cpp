// C++03 [class.union]/1 with [class.default.ctor]/1, /3: `s8(...)` and
// `s9(int = 0)` are default constructors, and neither is trivial although
// deleted, so a union member of such a type has a non-trivial default
// constructor (clang class.union/p1.cpp m8; WP-1090). `s10() = delete` is
// trivial, and a variadic copy constructor is non-trivial. clang's
// "because ..." notes are not produced (no strict-diags).
//@ expect: compile-fail
//@ std: c++98
//@ args: -Wno-c++11-extensions
//@ diag: 17:5 error "union member 'm8' has a non-trivial default constructor"
//@ diag: 20:5 error "union member 'm9' has a non-trivial default constructor"
//@ diag: 26:5 error "union member 'm11' has a non-trivial copy constructor"
//@ diag: 29:5 error "union member 'm12' has a non-trivial default constructor"
union U {
  struct s8 {
    s8(...) = delete;
  } m8;
  struct s9 {
    s9(int = 0) = delete;
  } m9;
  struct s10 {
    s10() = delete;
  } m10;
  struct s11 {
    s11(const s11 &, ...) = delete;
  } m11;
  struct s12 {
    template <class T> s12(T);
  } m12;
  int ok;
};
