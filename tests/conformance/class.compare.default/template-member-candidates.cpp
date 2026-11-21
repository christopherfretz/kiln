// [class.compare.default]/3, [class.eq]/2, [over.match.funcs]: a defaulted
// comparison's member comparisons are resolved with every candidate,
// function templates included; one whose constraints are not satisfied or
// whose deduction fails is not viable, so a member whose only `==` is such
// a template makes the defaulted `==` deleted, and one with a viable
// non-template `==` beside it is compared with that.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-defaulted-function-deleted
struct M {
  template <class T> requires (sizeof(T) == 0) bool operator==(const T &) const;
};
struct S { M m; bool operator==(const S &) const = default; };
template <class T> concept Eq = requires(const T &t) { t == t; };
static_assert(!Eq<S>);
struct M2 {
  int v;
  template <class T, class = typename T::nope>
  constexpr bool operator==(const T &) const { return false; }
  constexpr bool operator==(const M2 &o) const { return v == o.v; }
};
struct R { M2 n; constexpr bool operator==(const R &) const = default; };
static_assert(R{{1}} == R{{1}} && !(R{{1}} == R{{2}}));
