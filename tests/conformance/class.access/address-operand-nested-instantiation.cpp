// [class.access.base]/5, [class.protected], [temp.point]: a protected
// member named through a base class in a member function of a derived
// class is accessible through the implicit `*this`. Substituting the
// operand of a unary `&` (`&holder<T>::f`, `&M::constraints` in a template
// argument) instantiated a class whose member template's deduced return
// type needed its body, and that body's `base_t::m` was checked as if it
// formed a pointer to member, rejecting it.
// Reduced from real code (WP-1727, a regression since 1.2.1).
//@ expect: compile-ok
//@ std: c++20

template <class S> class base {
protected:
  S m{};
};
template <class S> class mid : public base<S> {};
template <class S> class derived : public mid<S> {
  using base_t = mid<S>;

public:
  template <class U> auto get(U) const { return base_t::m; }
};

template <class T> T declval_();

// A static member function's address taken through a dependent class.
template <class T> struct holder {
  using type = decltype(declval_<derived<T>>().get(0));
  static void f() {}
};
template <class T> void take_address() {
  auto p = &holder<T>::f;
  (void)p;
}

// The concept-check idiom: `&M::constraints` in a template argument of a
// deduced parameter.
template <class T> struct checked {
  using type = decltype(declval_<derived<T>>().get(0));
  void constraints() {}
};
template <class M, void (M::*)()> struct wrap {};
template <class M> char has_constraints(M *, wrap<M, &M::constraints> * = 0);
long has_constraints(...);
static_assert(sizeof(has_constraints((checked<long> *)0)) == 1);

int main() {
  take_address<int>();
  (void)derived<short>().get(0);
}
