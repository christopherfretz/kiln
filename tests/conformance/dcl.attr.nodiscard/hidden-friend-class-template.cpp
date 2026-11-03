// [dcl.attr.nodiscard]/2, [temp.inst]/4, [basic.lookup.argdep]/4 (WP-491):
// a nodiscard friend function defined in a class template (a hidden
// friend) is instantiated with the class, attribute included, so a
// discarded call found by argument-dependent lookup on the specialization
// warns (clang `InitFunctionInstantiation` -> `InstantiateAttrs`, friends
// included). libc++ shapes: `iter_move` of the filter and join_with view
// iterators, the iota/repeat iterator `operator+`/`operator-` (called by
// name here; the operator forms are in `hidden-friend-operator-*.cpp`), a
// friend function template, a unary operator and a message.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 42:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 43:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 44:3 warning "ignoring return value of function declared with 'nodiscard' attribute: distance"
//@ diag: 45:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 46:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 48:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 37:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 49:3 note "in instantiation of function template specialization 'dependent<long>' requested here"
template <class T> struct iterator {
  T v;
  [[nodiscard]] friend constexpr T iter_move(const iterator& i) noexcept(noexcept(T(i.v))) { return i.v; }
  [[nodiscard]] friend constexpr iterator operator+(iterator i, int n) { return {i.v + n}; }
  [[nodiscard("distance")]] friend constexpr int operator-(const iterator& a, const iterator& b) { return a.v - b.v; }
  [[nodiscard]] friend constexpr iterator operator-(iterator i) { return {-i.v}; }
  template <class U> [[nodiscard]] friend constexpr bool same(const iterator&, U) { return false; }
  friend constexpr int plain(const iterator& i) { return i.v; }
};

template <class T> struct view {
  struct sentinel {
    [[nodiscard]] friend constexpr bool at_end(const iterator<T>& i, const sentinel&) { return i.v == 0; }
  };
};

template <class T> void dependent(iterator<T> i) {
  iter_move(i);
}

void use() {
  iterator<int> i{1}, j{2};
  iter_move(i);
  operator+(i, 1);
  operator-(i, j);
  -i;
  same(i, 1.0);
  plain(i);
  at_end(i, view<int>::sentinel{});
  dependent(iterator<long>{3});
  static_assert(iter_move(iterator<int>{4}) == 4);
}
