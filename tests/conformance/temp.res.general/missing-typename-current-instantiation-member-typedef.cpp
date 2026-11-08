// [temp.res.general]/4-5, [temp.dep.type]/1 (WP-676): inside a class
// template's definition, `basic::jp::string_t` and `self::jp::string_t`
// name a member of the current instantiation's member typedef `jp`, whose
// type `ptr<S>` is dependent: without `typename` it is not a type, so a
// template type argument spelled that way is clang's "must be a type"
// error. The WP-668 lookup through a non-dependent member typedef of a
// specialization (`basic2<S>::m::string_t`, `m` = `ptr<int>`) still
// finds the type. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:25 error "template argument for template type parameter must be a type; did you forget 'typename'?"
//@ diag: 18:26 note "template parameter is declared here"
//@ diag: 29:25 error "template argument for template type parameter must be a type; did you forget 'typename'?"
//@ diag: 18:26 note "template parameter is declared here"
//@ diag: 32:49 error "template argument for template type parameter must be a type; did you forget 'typename'?"
//@ diag: 18:26 note "template parameter is declared here"
template <class R> struct ptr { using string_t = R; static constexpr int w = sizeof(R); };
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
template <int N> struct I {};
template <class S> struct basic {
  using jp = ptr<S>;
  using self = basic<S>;
  static_assert(same<S, typename basic<S>::jp::string_t>::value);
  static_assert(same<S, typename basic::jp::string_t>::value);
  using a1 = I<basic<S>::jp::w>;
  using a2 = I<basic::jp::w>;
  static_assert(same<S, self::jp::string_t>::value);
  static_assert(same<S, basic::jp::string_t>::value);
  void f();
};
template <class S> void basic<S>::f() { same<S, basic<S>::jp::string_t> x; }
template <class S> struct basic2 {
  using m = ptr<int>;
  static_assert(same<int, basic2<S>::m::string_t>::value);
  static_assert(same<int, basic2::m::string_t>::value);
};
basic<int> b;
basic2<char> b2;
