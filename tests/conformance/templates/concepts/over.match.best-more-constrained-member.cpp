//@ expect: compile-ok
// [over.match.best.general]/2.6: of two non-template member functions of
// a class template specialization with the same parameter-type-lists, the
// more constrained; the constraints are the pattern's, checked with the
// specialization's arguments ([temp.inst]/17). Constructors too, and
// redeclarations differing only in the trailing requires-clause declare
// distinct members ([temp.over.link]/5).
template <class T> concept Int = __is_same(T, int);
template <class T> concept Small = sizeof(T) <= 4;
template <class T> concept IntSmall = Int<T> && Small<T>;
template <class T> struct A {
  constexpr int f() const requires Int<T> { return 1; }
  constexpr int f() const requires IntSmall<T> { return 2; }
  constexpr int f() const { return 0; }
  constexpr int h(int) requires Int<T> { return 1; }
  constexpr int h(long) requires IntSmall<T> { return 2; }
};
static_assert(A<int>{}.f() == 2);
static_assert(A<long>{}.f() == 0);
static_assert(A<int>{}.h(1) == 1);

template <class T> struct Opt {
  int v = 0;
  constexpr Opt() = default;
  constexpr Opt(const Opt&) requires Int<T> = default;
  constexpr Opt(const Opt& o) requires (!Int<T>) : v(o.v + 1) {}
  constexpr Opt(int) requires Int<T> : v(10) {}
  constexpr Opt(int) requires IntSmall<T> : v(20) {}
};
constexpr Opt<int> a{};
constexpr Opt<int> b = a;
static_assert(b.v == 0);
constexpr Opt<long> c{};
constexpr Opt<long> d = c;
static_assert(d.v == 1);
constexpr Opt<int> e(1);
static_assert(e.v == 20);

// A member template of a class template specialization keeps its
// constraints, written in terms of the enclosing parameters.
template <class T> struct Box {
  template <class U> requires (sizeof(T) == sizeof(U)) static constexpr int same(U) { return 1; }
  template <class U> static constexpr int same(U) { return 2; }
  template <Int U> static constexpr int ci(U, T) { return 3; }
  template <class U> static constexpr int ci(U, T) { return 4; }
};
static_assert(Box<int>::same(1u) == 1);
static_assert(Box<int>::same(1.0) == 2);
static_assert(Box<long>::ci(1, 2L) == 3);
static_assert(Box<long>::ci(1L, 2L) == 4);

// A hidden friend with a trailing requires-clause.
template <class T> struct Eq {
  friend constexpr bool operator==(Eq, Eq) requires Int<T> { return true; }
  friend constexpr bool operator==(Eq, Eq) requires (!Int<T>) { return false; }
};
static_assert(Eq<int>{} == Eq<int>{});
static_assert(!(Eq<long>{} == Eq<long>{}));
