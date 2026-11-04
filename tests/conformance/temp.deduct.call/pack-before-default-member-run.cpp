// [temp.deduct.call]/1, [temp.param]/14: `sizeof...(Args)` of the
// specialization a member of a class template calls, for a pack that
// precedes a defaulted parameter: the number of call arguments, as for the
// same template at namespace scope; a non-static member template called
// through the implicit `this` and one whose parameter after the pack is
// deduced from the call (`template <class... A, class E> f(E, A...)`)
// keep their deduced arguments too (WP-629).
//@ expect: run
//@ stdout: 0 1 2 3
//@ stdout: 0 2 1
//@ stdout: 0 0 4
template <class T> struct H {
  template <class... Args, class E = int>
  static int mk(Args&&... pairs) { return sizeof...(Args); }
  template <class... Args, class E = int>
  int nm(Args...) { return sizeof...(Args); }
  template <class... A, class E>
  static int ded(E, A...) { return sizeof(E) == sizeof(int) ? sizeof...(A) : -1; }
  int m0() { return mk(); }
  int m1() { return mk(1); }
  int m2() { return mk(1, 2); }
  int m3() { return mk(1, 2, 3); }
  int n0() { return nm(); }
  int n2() { return nm(1, 2); }
  int n1() { return this->nm('a'); }
  int d0() { return ded(1); }
};
template <class... Args, class E = int> int fr(Args&&...) { return sizeof...(Args); }
extern "C" int printf(const char*, ...);
int main() {
  H<int> h;
  printf("%d %d %d %d\n", h.m0(), h.m1(), h.m2(), h.m3());
  printf("%d %d %d\n", h.n0(), h.n2(), h.n1());
  printf("%d %d %d\n", h.d0(), fr(), fr(1, 2, 3, 4));
}
