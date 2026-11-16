// C++11 [dcl.constexpr]/8: a constexpr non-static member function that is
// not a constructor is implicitly const (removed in C++14, N3652); clang's
// -Wconstexpr-not-const warns at each declaration outside template
// instantiation. The implicit const is part of the type: the object is
// const in the body, a const object can call it, and a const overload of
// the same signature redeclares it. A static member function, also defined
// out of line, is not affected. Found running GCC's C++ testsuite.
//@ std: c++11
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:17 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior"
//@ diag: 34:36 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior"
//@ diag: 35:17 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior"
//@ diag: 36:18 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior"
//@ diag: 38:17 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior"
//@ diag: 39:13 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior"
//@ diag: 36:33 error "binding reference of type 'int' to value of type 'const int' drops 'const' qualifier"
//@ diag: 38:34 error "'this' argument to member function 'm' has type 'const S', but function is not marked const"
//@ diag: 37:7 note "'m' declared here"
//@ diag: 41:18 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior"
//@ diag: 43:45 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior"
//@ diag: 47:17 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior"
//@ diag: 48:7 error "class member cannot be redeclared"
//@ diag: 47:17 note "previous declaration is here"
//@ diag: 52:16 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior"
//@ diag: 52:40 error "binding reference of type 'W' to value of type 'const W' drops 'const' qualifier"
struct S {
  int x;
  constexpr int f() { return x; }
  constexpr int g() const { return x; }
  constexpr S(int a) : x(a) {}
  static constexpr int h() { return 1; }
  static constexpr int h2();
  template <class T> constexpr int t(T) { return x; }
  constexpr int k();
  constexpr int &r() & { return x; }
  int m();
  constexpr int callm() { return m(); }
  constexpr operator long() { return x; }
};
constexpr int S::k() { return x; }
constexpr int S::h2() { return 2; }
template <class T> struct U { constexpr int f() { return 0; } };
U<int> u;
int use(const S &s) { return s.f() + s.k() + s.t(1) + u.f(); }
struct V {
  constexpr int f();
  int f() const;
};
struct W {
  int v;
  constexpr W &operator=(int) { return *this; }
};
