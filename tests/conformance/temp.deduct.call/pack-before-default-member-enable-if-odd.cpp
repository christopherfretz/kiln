// [temp.deduct.call]/1, [temp.param]/14: with one call argument the pack
// is `<int>` and the default `std::enable_if_t<1 % 2 == 0>` is a
// substitution failure, so the call from the class template's member has
// no viable candidate (WP-629).
//@ expect: compile-fail
//@ diag: 14:22 error "no matching function for call to 'mk'"
//@ diag: 13:14 note "was not satisfied [with Args = <int>]"
#include <type_traits>
// The pack precedes the defaulted parameter.
template <class T> struct H {
  template <class... Args,
            class E = std::enable_if_t<sizeof...(Args) % 2 == 0>>
  static int mk(Args&&... pairs) { return sizeof...(Args); }
  int odd() { return mk(1); }
};
int main() { H<int> h; return h.odd(); }
