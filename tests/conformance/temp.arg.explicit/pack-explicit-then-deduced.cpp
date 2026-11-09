// [temp.arg.explicit]/9: a pack given some explicit arguments is extended by
// deduction from the remaining ones. From clang's
// CXX/temp/temp.fct.spec/temp.arg.explicit/p9-0x.cpp. Found by the clang
// negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, deduction does not extend an explicitly specified pack
//@ expect: compile-ok
template <typename... Types> struct tuple {};
template <typename... Types> void accept_tuple(tuple<Types...>);
void test(tuple<int, float, double> t3) {
  accept_tuple(t3);
  accept_tuple<int, float, double>(t3);
  accept_tuple<int>(t3);
  accept_tuple<int, float>(t3);
}
