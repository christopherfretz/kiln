// Per-specialization entries (spec/ast.md §18.9, §18.7.4): an unqualified
// dependent call ([temp.dep.general]/2) finds its candidates by ordinary
// lookup at the definition and by argument-dependent lookup at
// instantiation ([temp.dep.candidate]/1). Each specialization's line has
// the function selected, `how=adl` when only argument-dependent lookup
// found it, `how=both` when ordinary lookup found it too, and the
// associated set of that specialization's argument types
// ([basic.lookup.argdep]/3): for `lib::Item`, namespace `lib` and the
// class; for `other::Thing<lib::Item>`, `other`, the class template
// specialization, then its template argument's class and namespace.
// A hidden friend ([class.friend]/7) is found by argument-dependent
// lookup only.
//@ written-refs: inst-adl.refs
void swap_in(int &, int &);

template <class T> void exchange(T &a, T &b) {
  swap_in(a, b);
}

namespace lib {
struct Item {
  friend void touch(Item &) {}
};
void swap_in(Item &, Item &);
} // namespace lib

namespace other {
template <class T> struct Thing {};
template <class T> void swap_in(Thing<T> &, Thing<T> &);
} // namespace other

template <class T> void poke(T &t) { touch(t); }

void run() {
  int i = 0, j = 1;
  exchange(i, j);
  lib::Item x, y;
  exchange(x, y);
  other::Thing<lib::Item> s, t;
  exchange(s, t);
  poke(x);
}
