// [temp.inst]/14, [temp.inst]/17, [except.spec]/13: the noexcept-specifier
// of a friend defined in a class template specialization is instantiated
// only when needed; a friend whose constraints are not satisfied is never
// selected, so its specification (here ill-formed for the arguments) is
// never substituted. libc++ filter_view / join_view `iter_swap`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:3 error "no matching function for call to 'iter_swap'"
//@ diag: 22:5 note "candidate function not viable: constraints not satisfied"
//@ diag: 23:16 note "because 'NoSwap' does not satisfy 'swappable_c'"
//@ diag: 12:61 note "because 'a.swap(a)' would be invalid: no member named 'swap' in 'NoSwap'"
template <class T> concept swappable_c = requires(T& a) { a.swap(a); };
struct Fn {
  template <class T> requires swappable_c<T>
  void operator()(T&, T&) const noexcept(sizeof(T) > 1) {}
};
inline constexpr Fn iter_swap_cpo{};
template <class V> struct view {
  template <bool C> struct iterator {
    V cur;
    friend void
    iter_swap(const iterator& x, const iterator& y) noexcept(noexcept(iter_swap_cpo(x.cur, y.cur)))
      requires swappable_c<V>
    {}
  };
};
struct NoSwap {};
struct Swap1 { char c; void swap(const Swap1&) const {} };
struct Swap4 { int i; void swap(const Swap4&) const {} };
template <class T> concept has_iter_swap = requires(T& a) { iter_swap(a, a); };

// Unsatisfied: not a candidate, and its specification is never needed.
static_assert(!has_iter_swap<view<NoSwap>::iterator<true>>);
static_assert(!has_iter_swap<view<NoSwap>::iterator<false>>);

// Satisfied: the specification is instantiated when the call needs it.
static_assert(has_iter_swap<view<Swap1>::iterator<true>>);
void f(view<Swap1>::iterator<false>& a, view<Swap4>::iterator<false>& b) {
  static_assert(!noexcept(iter_swap(a, a)));
  static_assert(noexcept(iter_swap(b, b)));
  iter_swap(a, a);
  iter_swap(b, b);
}
// Found by argument-dependent lookup and unsatisfied: a non-viable
// candidate ([over.match.viable]/3), its specification still not needed.
void g(view<NoSwap>::iterator<false>& n) {
  iter_swap(n, n);
}
