// [temp.deduct.call]/1, [temp.param]/14: a template parameter pack that is
// not the last template parameter is deduced from the call, and the
// defaulted parameter after it takes its default after deduction. A call
// of such a member template from a member of a class template is resolved
// at the definition and named again in the instantiated class: the
// specialization keeps its deduced arguments (`Args = <>`, `E = int`),
// not the flattened list re-read as explicit arguments, which would give
// the pack the default (WP-629). Matches clang 19.
template <class T> struct H {
  template <class... Args, class E = int>
  static int mk(Args&&... pairs) { return sizeof...(Args); }
  int direct() { return mk(); }
  int direct2() { return mk(1); }
  int direct3() { return H::mk(); }
};
template <class... Args, class E = int> int fr(Args&&...) { return 0; }
int g() { return fr(); }
int main() { H<int> h; return h.direct() + h.direct2() + h.direct3() + g(); }
