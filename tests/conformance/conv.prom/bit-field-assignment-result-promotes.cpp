// [conv.prom]/5, [expr.assign]/1: the result of assigning to a bit-field is
// that bit-field, so unary + promotes an 'unsigned : 2' result to int. From
// clang's SemaCXX/bitfield.cpp. Found by the clang negative-test run
// (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, an assignment or increment result loses its bit-field-ness for promotion
//@ expect: compile-ok
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
struct T {
  unsigned n : 2;
} t;
static_assert(same<decltype(+t.n), int>, "");
static_assert(same<decltype(+(t.n = 0)), int>, "");
static_assert(same<decltype(+(t.n += 0)), int>, "");
static_assert(same<decltype(+(++t.n)), int>, "");
