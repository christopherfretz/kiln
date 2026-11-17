// [expr.prim.this]/2: accepted side of static-member-declarator-errors.cpp.
// A non-static member function has `this` in its trailing return type and
// *noexcept-specifier*, with its cv-qualifiers; a static member function
// may name a static member, or a non-static data member where no `this`
// is formed (a parameter's type, a default argument's or body's unevaluated
// operand); an explicit object member function uses its object parameter.
// Found running GCC's C++ testsuite.
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
struct S {
  int m;
  int f() const;
  auto a() const -> decltype(this->m);
  auto b() -> decltype(f());
  auto c() const noexcept(noexcept(f())) -> int;
  static int sf();
  static auto d() -> decltype(sf());
  static int e(decltype(m));
  static int g(int = sizeof(m));
  static int h() { return sizeof(m) + sf(); }
  template <class T> auto k(T t) -> decltype(t + m);
  int self(this S const& s) { return s.m; }
  auto x(this S const& s) -> decltype(s.m);
};
template <class T> struct TS {
  T m;
  auto a() const -> decltype(this->m);
  static auto d() -> decltype(sizeof(T));
};
TS<int> ts;
auto S::b() -> decltype(f()) { return 0; }
static_assert(__is_same(decltype(S().a()), int));
