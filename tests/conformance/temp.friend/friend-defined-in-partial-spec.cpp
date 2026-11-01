// [temp.friend], [temp.inst]/4: a friend function defined in a class
// template, and the same friend defined in a partial specialization of it,
// do not redeclare each other before instantiation; only one of them is
// instantiated per specialization (libc++ <expected> and its `void`
// partial specialization both define `operator==(const expected&, const
// unexpected<_E2>&)`). Verified against clang 19.
//@ expect: run
template <class E> struct U { E e; };
template <class T, class E> struct X {
  template <class E2> friend constexpr bool operator==(const X&, const U<E2>&) { return false; }
  friend constexpr int tag(const X&) { return 1; }
};
template <class T, class E> struct X<T*, E> {
  template <class E2> friend constexpr bool operator==(const X&, const U<E2>&) { return true; }
  friend constexpr int tag(const X&) { return 2; }
};
static_assert(!(X<int, int>{} == U<int>{}));
static_assert(X<int*, int>{} == U<long>{});
static_assert(tag(X<int, int>{}) == 1 && tag(X<int*, int>{}) == 2);
int main() { return tag(X<char, int>{}) == 1 ? 0 : 1; }
