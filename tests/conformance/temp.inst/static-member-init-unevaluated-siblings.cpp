// [temp.inst]/4, /8: `decltype`, `noexcept` and `sizeof` name a
// constexpr static data member or variable template
// specialization without instantiating its initializer; only the
// declaration's type is needed (WP-1497).
//@ expect: compile-ok
//@ std: c++20
template <class T> struct A { static constexpr int value = T::nonexistent; };
template <class T> constexpr int v = T::nonexistent;
template <int> struct B {};
using t1 = decltype(A<int>::value);
using t2 = decltype(v<int>);
constexpr bool b1 = noexcept(A<int>::value);
constexpr auto s1 = sizeof(A<int>::value) + sizeof(v<int>);
