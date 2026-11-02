// [temp.dep.expr]/3, [temp.dep.candidate], [over.match.call]: in a
// member of a class template, an unqualified or qualified name for an
// overload set of static member functions one of which has a dependent
// type (`key(const node<K>*)`) is type-dependent, so the call
// `key(p)` is resolved at instantiation, where `key(const nb*)` is the
// only viable candidate for an `nb*` argument (an `nb*` does not
// convert to `const node<int>*`). Resolving it at the definition, where
// the dependent parameter's conversion is unknown, picked the first
// candidate. libstdc++ bits/stl_tree.h `_Rb_tree::_S_key` called with a
// `_Base_ptr` from `_M_insert_` / `_M_insert_node` (std::map::insert,
// emplace; WP-366).
// Verified against clang 19.
//@ expect: run
struct nb {};
template<class V> struct node : nb { V v; };

template<class K> struct tree {
  using bp = nb*;
  static int key(const node<K>*) { return 1; }
  static int key(const nb*) { return 2; }
  int in_class(bp p) { return key(p); }
  static int from_static(bp p, node<K>* z) { return tree::key(p) * 10 + key(z); }
  int out_of_line(bp p, node<K>* z);
};

template<class K> int tree<K>::out_of_line(bp p, node<K>* z) { return key(z) * 10 + key(p); }

int main() {
  tree<int> t;
  nb a;
  node<int> z{};
  return t.in_class(&a) == 2 && tree<int>::from_static(&a, &z) == 21
                 && t.out_of_line(&a, &z) == 12
             ? 0
             : 1;
}
