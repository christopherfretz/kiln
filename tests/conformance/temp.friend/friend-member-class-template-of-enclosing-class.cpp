// [temp.friend]/1, [namespace.memdef]/3, [dcl.type.elab]/4: the unqualified
// name in `template <bool> friend class S;` is looked up up to the innermost
// enclosing namespace, so in a nested class it finds a member class template
// of an enclosing class (or the injected-class-name of the class itself),
// and every specialization of that template is a friend (libc++
// elements_view.h: `__iterator` befriends `template <bool> class __sentinel`,
// whose `__get_current` reads `__iterator<C>::__current_`). A class template
// of the same name at namespace scope is a different entity.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 55:41 error "'c' is a private member of"
//@ diag: 50:50 note "declared private here"
template <class V> struct ev {
  template <bool> class iterator;
  template <bool> class sentinel;
};
template <class V>
template <bool C>
class ev<V>::iterator {
  template <bool> friend class iterator;
  template <bool> friend class sentinel;
  int cur_ = 0;
public:
  template <bool O> int other(const iterator<O>& o) const { return o.cur_; }
};
template <class V>
template <bool C>
class ev<V>::sentinel {
public:
  template <bool A>
  static constexpr int get(const iterator<A>& it) { return it.cur_; }
};

struct plain {
  template <bool> class s;
  class it {
    template <bool> friend class s;
    int c = 0;
  };
  template <bool> class s {
  public:
    static int g(it& x) { return x.c; }
  };
};

// Befriends `outer::s`; the namespace-scope `s` gains nothing.
template <bool> class s;
struct outer {
  template <bool> class s;
  class it { template <bool> friend class s; int c = 0; };
  template <bool> class s { public: static int g(it& x) { return x.c; } };
};
template <bool> class s {
public:
  static int g(outer::it& x) { return x.c; }
};

int main() {
  ev<int>::iterator<false> i;
  ev<int>::iterator<true> j;
  plain::it p;
  outer::it o;
  return ev<int>::sentinel<true>::get(i) + j.other(i) + plain::s<false>::g(p) +
         outer::s<true>::g(o);
}
