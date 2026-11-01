// [dcl.type.elab]/4, [temp.friend]/1: `template<class> friend struct N::X;`
// befriends the class template that qualified lookup finds (libc++'s
// `friend struct std::__segmented_iterator_traits`), and
// `template<bool> friend class A<T>::B;` a member template of the
// current instantiation (libc++'s `take_view<_View>::__sentinel`).
//@ std: c++26
namespace N { template <class T> struct Tr; }

template <class V> class JV {
  int secret = 7;
  template <class> friend struct N::Tr;
};
template <class T> struct N::Tr {
  static int get(JV<T> &j) { return j.secret; }
};
int a(JV<int> &j) { return N::Tr<int>::get(j); }

class C {
  int x = 1;
  template <class> friend class N::Tr;
};
namespace N {
template <> struct Tr<C> { static int g(C &c) { return c.x; } };
}
int c(C &v) { return N::Tr<C>::g(v); }

template <class V> struct TV {
  template <bool> class S;
};
template <class V> template <bool B> class TV<V>::S {
  int p = B;
  template <bool> friend class TV<V>::S;

public:
  int peek(S<!B> &o) { return o.p; }
};
int b(TV<int>::S<true> &s, TV<int>::S<false> &f) { return s.peek(f) + f.peek(s); }
