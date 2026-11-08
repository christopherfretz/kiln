// [temp.names]/4 (C++11, N1757), [expr.type.conv]/1: after `> >` closes
// the inner argument lists, `V<V<int> >::type::type(3)` is a functional
// cast to `int` and so a valid non-type template argument; it does not
// name a value-less `type`. WP-647 (4); already accepted on main when
// WP-851 pinned it.
//@ expect: compile-ok
//@ std: c++11
//@ strict-diags
template <class T> struct V { typedef T type; };
template <int N> struct I { static const int value = N; };
static_assert(I<V<V<int> >::type::type(3)>::value == 3, "nested functional cast");
static_assert(I<V<V<int>>::type::type(4)>::value == 4, ">> then functional cast");
static_assert(I<V<int>::type(5)>::value == 5, "functional cast");
