// [temp.spec.partial.member]/1: the members of a class template partial specialization defined
// out of line; `V<bool, A>::` names the partial specialization, and its member names are found in
// the parameter list and the body ([temp.dep.type]/1.2)
//@ expect: run
//@ strict-diags
template <class T> struct ti { using type = T; };
template <class T> using ti_t = typename ti<T>::type;
template <class T, class A> struct V { int f() { return 1; } };
template <class A> struct V<bool, A> {
  using allocator_type = A;
  allocator_type a;
  static int count;
  V(allocator_type x) : a(x) {}
  V(V&& o, const ti_t<allocator_type>& extra);
  int f();
  allocator_type get(allocator_type d) const;
};
template <class A> int V<bool, A>::count = 40;
template <class A> V<bool, A>::V(V&& o, const ti_t<allocator_type>& extra) : a(o.a + extra) {}
template <class A> int V<bool, A>::f() { allocator_type x = a; return int(x) + count; }
template <class A> auto V<bool, A>::get(allocator_type d) const -> allocator_type { return a + d; }
int main() {
  V<bool, int> v(1);
  V<bool, int> w(static_cast<V<bool, int>&&>(v), 2);
  V<char, int> p;
  return (w.f() == 43 && w.get(4) == 7 && p.f() == 1) ? 0 : 1;
}
