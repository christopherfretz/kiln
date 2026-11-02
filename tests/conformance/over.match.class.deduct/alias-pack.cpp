// [over.match.class.deduct]/3: the guides of an alias template whose
// defining-type-id holds a pack expansion (`tuple<int, T...>`). Each guide
// of `tuple` is deduced against it; an element of a deduced pack that is
// itself the expansion `T...` keeps the function parameter a pack
// (`const U&...` with `U = <int, T...>` is `const int&, const T&...`). An
// alias template's guide formed from a user-declared guide keeps that
// guide's precedence over the implicit ones
// ([over.match.best.general]/2.11), so they do not tie.
//@ expect: compile-ok
//@ strict-diags
template<class A, class B> struct same { static constexpr bool v = false; };
template<class A> struct same<A, A> { static constexpr bool v = true; };
template<class A, class B> struct pair { A a; B b; };
struct alloc_tag {};

template<class... T> struct tuple {
  tuple(const T&...) {}
  template<class Alloc> tuple(alloc_tag, const Alloc&, const T&...) {}
  template<class A, class B> tuple(const pair<A, B>&) {}
};
template<class... T> tuple(T...) -> tuple<T...>;
template<class A, class B> tuple(pair<A, B>) -> tuple<A, B>;
template<class Alloc, class... T> tuple(alloc_tag, Alloc, T...) -> tuple<T...>;

template<class... T> using TT = tuple<int, T...>;
TT t{1, 2.0, 'c'};
static_assert(same<decltype(t), tuple<int, double, char>>::v);
TT t1{1};
static_assert(same<decltype(t1), tuple<int>>::v);
TT t2(alloc_tag{}, 0, 1, 2L);
static_assert(same<decltype(t2), tuple<int, long>>::v);

