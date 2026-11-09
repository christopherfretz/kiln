// [except.spec]/8: valid twin of exception-spec-implicit.cpp. Implicit
// destructors and assignment operators whose callees are all non-throwing
// are noexcept and may override noexcept functions; a potentially-throwing
// overrider of a potentially-throwing function is fine (WP-957).
//@ std: c++17
struct D1;
struct B {
  virtual D1 &operator=(const D1 &) noexcept;
  virtual D1 &operator=(D1 &&) noexcept;
  virtual ~B() noexcept;
};
struct M { M &operator=(const M &) noexcept; ~M(); };
struct D1 : B { M m; };
struct A1 { virtual void f(); virtual ~A1() noexcept(false); };
struct T { ~T() noexcept(false); };
struct C : A1 { void f(); T t; };
template <bool V> struct E : B { void g() noexcept(V); ~E() noexcept(!V); };
E<false> e;
