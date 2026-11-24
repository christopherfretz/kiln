// [temp.friend]/1: a friend class template whose qualifier is the current
// instantiation, in a member class template defined outside its class
// (`template <bool> friend class take<V>::sentinel;`): the enclosing
// heads match the qualifier's class template, so `template <bool>` is the
// friend's own head and each specialization of the member befriends every
// other. Reduced from real code.
//@ expect: run
//@ stdout: 7
template <class V> struct take {
  template <bool C> class sentinel;
  template <bool C> class iter;
};
template <class V> template <bool C> class take<V>::sentinel {
  template <bool> friend class take<V>::sentinel;
  template <bool> friend class take<V>::iter;
  int x = 7;

public:
  template <bool D> int peek(const sentinel<D>& o) const { return o.x; }
};
template <class V> template <bool C> class take<V>::iter {};
int main() {
  take<int>::sentinel<true> a;
  take<int>::sentinel<false> b;
  __builtin_printf("%d\n", a.peek(b));
}
