// [temp.deduct.call]/1, [temp.param]/14: two defaulted parameters after a
// non-trailing pack (`template <class... A, class B = int, class C =
// long>`) both take their defaults after `A` is deduced from the call, in
// a call from a member of a class template as at namespace scope
// (WP-629).
//@ expect: run
//@ stdout: 0 1 2 1
//@ stdout: 1 1 1 1
template <class T> struct H {
  template <class... A, class B = int, class C = long>
  static int two(A...) {
    static_assert(__is_same(B, int) && __is_same(C, long));
    return sizeof...(A);
  }
  template <class... A, class B = int, class C = long>
  static bool defaults(A...) { return __is_same(B, int) && __is_same(C, long); }
  int f0() { return two(); }
  int f1() { return two(1); }
  int f2() { return two(1, 'a'); }
  int f3() { return H::two(nullptr); }
  bool g0() { return defaults(); }
  bool g1() { return defaults(1); }
  bool g2() { return defaults(1, 2); }
  bool g3() { return H::defaults(); }
};
extern "C" int printf(const char*, ...);
int main() {
  H<char> h;
  printf("%d %d %d %d\n", h.f0(), h.f1(), h.f2(), h.f3());
  printf("%d %d %d %d\n", h.g0(), h.g1(), h.g2(), h.g3());
}
