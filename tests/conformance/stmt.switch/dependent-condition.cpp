// [stmt.switch]/2, [temp.res.general]/6: the condition of a switch whose
// type depends on a template parameter is converted and promoted, and its
// case values are evaluated, when the template is instantiated (libc++
// <__functional/hash.h> switches on a `_Size` length).
//@ expect: compile-ok
//@ strict-diags
enum class E : unsigned char { z, a, b };

template <class Size>
constexpr Size tail(Size len) {
  Size h = 0;
  switch (len) {
  case 3:
    h += 100;
    [[fallthrough]];
  case 2:
    h += 10;
    [[fallthrough]];
  case 1:
    h += 1;
    break;
  default:
    h = 7;
  }
  return h;
}
static_assert(tail<unsigned long>(3) == 111);
static_assert(tail<int>(2) == 11);
static_assert(tail<short>(9) == 7);

template <class T, T V>
constexpr int pick(T v) {
  switch (v) {
  case V:
    return 1;
  case T{}:
    return 2;
  default:
    return 3;
  }
}
static_assert(pick<E, E::b>(E::b) == 1);
static_assert(pick<E, E::b>(E{}) == 2);
static_assert(pick<char, 'x'>('y') == 3);

template <class T>
struct S {
  static constexpr int f(T t) {
    switch (t) {
    case sizeof(T):
      return 1;
    default:
      return 0;
    }
  }
};
static_assert(S<long>::f(8) == 1 && S<char>::f(0) == 0);
