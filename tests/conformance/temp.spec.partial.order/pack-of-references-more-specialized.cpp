// [temp.spec.partial.order], [temp.deduct.partial]: X1<tuple<Head&,
// Tail&...>> is more specialized than X1<tuple<Head, Tail&...>>. From
// clang's CXX/temp/temp.decls/temp.variadic/partial-ordering.cpp. Found by
// the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-ok
template <typename... Types> struct tuple;
template <typename Tuple> struct X1 {
  static const unsigned value = 0;
};
template <typename Head, typename... Tail> struct X1<tuple<Head, Tail &...>> {
  static const unsigned value = 2;
};
template <typename Head, typename... Tail> struct X1<tuple<Head &, Tail &...>> {
  static const unsigned value = 3;
};
static_assert(X1<tuple<int &, int &>>::value == 3, "");
