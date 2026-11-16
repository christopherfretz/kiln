// C++11 [dcl.constexpr]/8: a constexpr non-static member function that is
// not a constructor is const (removed in C++14), with clang's
// -Wconstexpr-not-const. It can be called on a constant object and
// overloads with a `const volatile` member; a static member, a constructor
// and one already const are left alone. Its body cannot modify a member.
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 19:17 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior [-Wconstexpr-not-const]"
//@ diag: 21:36 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior [-Wconstexpr-not-const]"
//@ diag: 25:13 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior [-Wconstexpr-not-const]"
//@ diag: 26:17 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior [-Wconstexpr-not-const]"
//@ diag: 26:32 error "cannot assign to non-static data member within const member function 'inc'"
//@ diag: 26:17 note "member function 'A::inc' is declared const here"
//@ diag: 35:40 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior [-Wconstexpr-not-const]"

struct A {
  int v;
  constexpr int get() { return v; }
  int get() const volatile;
  template <class T> constexpr int at(T) { return v; }
  static constexpr int s() { return 1; }
  constexpr A(int x) : v(x) {}
  constexpr int k() const { return v; }
  constexpr operator int() { return v; }
  constexpr int inc() { return ++v; }
};
constexpr A a(1);
static_assert(a.get() == 1, "");
static_assert(a.at(0) == 1, "");
static_assert(int(a) == 1, "");

template <class F> struct D {
  F f;
  template <class... T> constexpr auto operator()(const T&... t) -> decltype(f(t...)) { return f(t...); }
};
constexpr int twice(int x) { return 2 * x; }
constexpr D<int (*)(int)> d{&twice};
static_assert(d(3) == 6, "");
