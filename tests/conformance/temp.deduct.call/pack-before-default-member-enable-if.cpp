// [temp.deduct.call]/1, [temp.param]/14: the defaulted parameter after a
// non-trailing pack is `std::enable_if_t<sizeof...(Args) % 2 == 0>`; the
// call from a member of the class template deduces `Args` from the call
// arguments only, so the default is computed with the deduced pack and is
// never stored as an element of it (WP-629). Matches clang 19.
#include <type_traits>
template <class T> struct H {
  template <class... Args, class E = std::enable_if_t<sizeof...(Args) % 2 == 0>>
  static int mk(Args&&... pairs) { return sizeof...(Args); }
  int none() { return mk(); }
  int pair() { return mk(1, 2); }
  int pairs() { return mk(1, 'a', 2L, 3.0); }
};
int main() {
  H<int> h;
  return h.none() + h.pair() + h.pairs() == 6 ? 0 : 1;
}
