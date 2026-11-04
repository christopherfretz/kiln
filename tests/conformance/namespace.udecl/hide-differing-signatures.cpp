// [namespace.udecl]/14 (negative): a derived member function (template)
// whose parameter-type-list, cv-qualification or ref-qualifier differs
// from a using-declared base one, or which is a template where the base
// one is not, does not hide it; both are candidates and overload
// resolution picks the better one. WP-612.
//@ expect: run
//@ strict-diags
template <class D> struct DenseBase { int v; };
struct X : DenseBase<X> {};
struct B {
  template <class O> int f(const DenseBase<O>&) { return 1; }
  int g(long) const { return 1; }
  int h() const { return 1; }
  int r() & { return 1; }
  template <class O> int t(O) { return 1; }
  int n(int) { return 1; }
};
template <class T> struct D : B {
  using B::f; using B::g; using B::h; using B::r; using B::t; using B::n;
  template <class O> int f(DenseBase<O>&) { return 2; }
  int g(int) const { return 2; }
  int h() { return 2; }
  int r() && { return 2; }
  int t(int) { return 2; }
  template <class O> int n(O) { return 2; }
};
int main() {
  D<int> d;
  const D<int>& cd = d;
  X x;
  const X& cx = x;
  if (d.f(x) != 2 || d.f(cx) != 1) return 1;
  if (d.g(1) != 2 || d.g(1L) != 1) return 2;
  if (d.h() != 2 || cd.h() != 1) return 3;
  if (d.r() != 1 || static_cast<D<int>&&>(d).r() != 2) return 4;
  if (d.t(1) != 2 || d.t('c') != 1) return 5;
  if (d.n(1) != 1 || d.n('c') != 2) return 6;
  return 0;
}
