// [dcl.attr.nodiscard]/2, [over.match.oper], [basic.lookup.argdep]/4
// (WP-491): a nodiscard hidden friend operator of a class template, called
// with operator syntax, warns at the expression's start (clang 22). libc++ shapes: the iota and
// repeat view iterators' `operator+`/`operator-`, the join_with
// iterator's `operator==` (a discarded comparison that is not the
// statement itself is checked as the call it is).
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
//@ diag: 25:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 26:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 27:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 28:6 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 29:16 warning "expression result unused"
template <class T> struct iterator {
  T v;
  [[nodiscard]] friend constexpr iterator operator+(iterator i, int n) { return {i.v + n}; }
  [[nodiscard]] friend constexpr iterator operator+(int n, iterator i) { return {i.v + n}; }
  [[nodiscard]] friend constexpr T operator-(const iterator& a, const iterator& b) { return a.v - b.v; }
  [[nodiscard]] friend constexpr bool operator==(const iterator& a, const iterator& b) { return a.v == b.v; }
};

void use(bool c) {
  iterator<int> i{1}, j{2};
  i + 1;
  1 + i;
  i - j;
  (i == j);
  c ? i == j : false;
}
