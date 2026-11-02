// [class.friend]/2, [class.friend]/7, [class.access.nest]/1, [temp.friend]/1:
// a friend defined in a nested class is in that class's scope, so it has
// the access of the members of the enclosing classes, including friendship
// granted to an enclosing class. This holds for a friend function template
// and each of its specializations (libc++ join_view.h: `__sentinel`'s
// `operator==` calls `__iterator<C>::__get_outer()`, private, `__iterator`
// befriending `join_view`). A friend defined out of the class, or a class
// that does not befriend the enclosing class, grants nothing.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 55:44 error "'get' is a private member of"
//@ diag: 52:9 note "declared private here"
//@ diag: 65:71 error "'get' is a private member of"
//@ diag: 74:80 note "in instantiation of function template specialization"
//@ diag: 61:9 note "declared private here"
template <class V> struct view {
  template <bool C> struct iterator;
  template <bool C> struct sentinel {
    int end_ = 0;
    template <bool O>
    friend bool operator==(const iterator<O>& x, const sentinel& y) {
      return x.get() == y.end_;
    }
  };
  template <bool C> struct iterator {
    friend view;
  private:
    int get() const { return 0; }
  };
};

struct plain {
  struct iterator;
  struct sentinel {
    template <class U> friend int h(const iterator& x, sentinel, U) { return x.get(); }
    friend int k(const iterator& x, sentinel) { return x.get(); }
  };
  struct iterator {
    friend plain;
  private:
    int get() const { return 0; }
  };
};

// Out of the class: the definition is not in `sentinel`'s scope.
struct outer {
  struct iterator;
  struct sentinel { friend int m(const iterator&); };
  struct iterator {
    friend outer;
  private:
    int get() const { return 0; }
  };
};
int m(const outer::iterator& x) { return x.get(); }

// No `friend lonely;`: nothing to inherit.
template <class V> struct lonely {
  template <bool C> struct iterator {
  private:
    int get() const { return 0; }
  };
  template <bool C> struct sentinel {
    template <bool O>
    friend bool operator==(const iterator<O>& x, sentinel) { return x.get() == 0; }
  };
};

int main() {
  view<int>::iterator<false> i;
  view<int>::sentinel<true> s;
  plain::iterator p;
  lonely<int>::iterator<true> li;
  return (i == s) + h(p, plain::sentinel{}, 0) + k(p, plain::sentinel{}) + (li == lonely<int>::sentinel<false>{});
}
