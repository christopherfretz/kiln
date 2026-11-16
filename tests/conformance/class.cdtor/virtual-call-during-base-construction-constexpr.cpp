// [class.cdtor]/4, [expr.const]: in constant evaluation, as at run time,
// a virtual call during the construction of a base subobject uses the
// base as the dynamic type: from the base's constructor body, from a member
// initializer's argument, and from a later base's constructor through its
// own `this`. Found running GCC's C++ testsuite. Matches clang 22 and
// GCC 16.1.
//@ std: c++20
//@ strict-diags
struct V {
  char seen = 0;
  constexpr virtual char n() const { return 'V'; }
  constexpr V() { seen = n(); }
};
struct A : V {
  char arg;
  constexpr char n() const override { return 'A'; }
  static constexpr char pr(const V* v) { return v->n(); }
  constexpr A() : arg(pr(this)) {}
};
struct B {
  char seen = 0;
  constexpr virtual char m() const { return 'B'; }
  constexpr B() { seen = m(); }
};
struct C : A, B {
  char body = 0;
  constexpr char n() const override { return 'C'; }
  constexpr char m() const override { return 'c'; }
  constexpr C() { body = n(); }
};
constexpr C c;
static_assert(c.V::seen == 'V');
static_assert(c.arg == 'A');
static_assert(c.B::seen == 'B');
static_assert(c.body == 'C');
static_assert(c.n() == 'C' && c.m() == 'c');
