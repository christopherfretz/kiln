// [temp.arg.explicit]/7, /9, [temp.deduct.call]/4: a parameter type that
// deduces an explicitly specified pack stays dependent (clang retains the
// expansion: `tuple<T...>` with `T = {int}` given is `tuple<int, T...>`), so
// it is still deduced from and no implicit conversion applies to its
// argument. WP-958.
//@ std: c++20
//@ expect: compile-fail
//@ diag: 13:19 error "no matching function for call to 'a'"
template <class... T> struct tuple {};
template <class... T> int a(tuple<T...>) { return 0; }
struct C { operator tuple<int>(); };
int ok() { return a<int>(tuple<int>()); }
int t2() { return a<int>(C()); }
