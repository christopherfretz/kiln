// [basic.def.odr]/5: a variable named by an element of the potential
// results of a discarded-value expression to which the lvalue-to-rvalue
// conversion is not applied is not odr-used by it, also when it is a
// `constexpr` object of class type: the operand of a cast to void
// ([expr.static.cast]/6), an expression statement, either operand of a
// comma, through parentheses and a glvalue conditional. Not odr-used, the
// object gets no closure member: the text still captures it
// ([expr.prim.lambda.capture]/7) and lets the closure type omit the
// member ([expr.prim.lambda.closure]/2, its size and layout). A
// by-reference default, a nested lambda and a function template's lambda
// follow; an explicit capture and a generic lambda's body still capture.
// A constexpr reference is never odr-used by a name. Found running GCC's
// C++ testsuite; decided to follow the text (GCC 16.1 agrees, except that
// it captures `mu` and `rc` and gives the enclosing lambda of a nested one
// a member; clang 19 and 22 capture: the closure's size differs, a
// recorded deviation).
//@ xfail(clang): clang 19 and 22 capture a constant class object named only by a discarded-value expression
//@ expect: compile-ok
//@ args: -Wno-unused-value
struct C { int m; int n; };
struct Mu { mutable int m; };
static constexpr C sc{5, 6};
template <class T> int tf() {
  constexpr C ca{1, 2};
  auto a = [=] { (void)ca; };
  static_assert(sizeof(a) == 1);
  return 0;
}
int g(bool b) {
  constexpr C ca{1, 2}, cb{3, 4};
  constexpr Mu mu{0};
  constexpr const C& rc = sc;
  auto a = [=] { (void)ca; };
  static_assert(sizeof(a) == 1);
  auto c = [=] { ca; (ca); 0, ca; ca, 0; };
  static_assert(sizeof(c) == 1);
  auto d = [=] { (void)(b ? ca : cb); };
  static_assert(sizeof(d) == sizeof(bool));
  auto e = [=] { (void)mu; (void)rc; };
  static_assert(sizeof(e) == 1);
  auto f = [&] { (void)ca; };
  static_assert(sizeof(f) == 1);
  auto h = [=] { return [=] { (void)ca; }; };
  static_assert(sizeof(h) == 1 && sizeof(h()) == 1);
  auto i = [ca] { (void)ca; };
  static_assert(sizeof(i) == sizeof(C));
  auto j = [=](auto) { (void)ca; };
  static_assert(sizeof(j) == sizeof(C));
  return tf<int>();
}
