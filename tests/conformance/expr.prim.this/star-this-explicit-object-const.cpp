// [expr.prim.this]/2, [expr.prim.lambda.closure]/5, [temp.dep.expr]/3
// (P0847): in a lambda that captures `*this` and has an explicit object
// parameter, a member `x` of the copy is const exactly when the object
// parameter's referred type is, so with a deduced parameter
// `decltype((x))` depends on each specialization. A `[this]` capture
// names the enclosing object, whose constness the parameter does not
// change. Found running GCC's C++ testsuite (WP-1472). (clang 19 and 22
// crash generating code for the calls; this checks the types only.)
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
template <class T> constexpr bool is_cref = false;
template <class T> constexpr bool is_cref<const T &> = true;
template <class T> T &&mv(T &t) { return static_cast<T &&>(t); }

struct S {
  int x;
  auto fwd() {
    return [*this](this auto &&self) {
      constexpr bool c = is_cref<decltype((self))>;
      static_assert(same<decltype((x)), const int &> == c);
      static_assert(same<decltype((this->x)), decltype((x))>);
      return c;
    };
  }
  auto cref() { return [*this](this const auto &) { static_assert(same<decltype((x)), const int &>); }; }
  auto val() { return [*this](this auto) { static_assert(same<decltype((x)), int &>); }; }
  auto ref() { return [this](this const auto &) { static_assert(same<decltype((x)), int &>); }; }
  auto cref_outer() const { return [this](this auto &) { static_assert(same<decltype((x)), const int &>); }; }
  auto nested() {
    return [*this](this const auto &) {
      return [=](this auto &) { static_assert(same<decltype((x)), const int &>); };
    };
  }
};

void use() {
  S s{1};
  auto l = s.fwd();
  const auto cl = l;
  (void)l();
  (void)cl();
  (void)mv(l)();
  s.cref()();
  s.val()();
  s.ref()();
  auto o = s.cref_outer();
  o();
  auto n = s.nested()();
  n();
}
