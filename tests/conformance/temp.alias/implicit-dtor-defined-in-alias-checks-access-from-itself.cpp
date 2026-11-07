// [temp.alias]/2, [class.dtor]/14, [class.protected]: an alias template
// substituted at namespace scope checks its own accesses from the
// context of its use, but an implicit destructor defined meanwhile (the
// temporary of a `noexcept` operand) checks the destructors of its
// subobjects from itself: a protected base destructor is accessible,
// and the SFINAE default template argument through the alias succeeds
// (WP-789).
//@ expect: compile-ok
//@ std: c++20
template <class T> T&& declval();

struct base {
protected:
  ~base() = default;
};

template <class R> struct op {
  struct type : base {
    struct cb {
      ~cb() {}
    } c;
  };
};

struct sender {
  template <class R> typename op<R>::type connect(R) { return {}; }
};

struct connect_fn {
  template <class S, class R>
  auto operator()(S&& s, R&& r) const noexcept(noexcept(((S&&)s).connect((R&&)r)))
      -> decltype(((S&&)s).connect((R&&)r)) {
    return ((S&&)s).connect((R&&)r);
  }
};

template <class S, class R> using result_t = decltype(connect_fn{}(declval<S>(), declval<R>()));

template <class S, class R, class = result_t<S, R>> char probe(int);
template <class S, class R> long probe(...);
static_assert(sizeof(probe<sender, int>(0)) == 1);

using direct = result_t<sender, long>;
