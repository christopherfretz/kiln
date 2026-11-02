// [temp.mem.func]/1, [temp.local]/1, [temp.friend]/1, [temp.over.link]/5:
// a class template first declared by a friend declaration in a partial
// specialization is a templated friend (it joins the redeclaration chain
// only when the partial specialization is instantiated), so the
// injected-class-name of the later definition is spelled with the
// definition's own template parameters (depth 0), and the out-of-line
// `tree<K, V, A>& tree<K, V, A>::operator=(const tree&)` matches the
// in-class `tree& operator=(const tree&)`. The friend still grants
// access once the partial specialization is instantiated.
// libstdc++ bits/node_handle.h (`_Node_handle<_Value, _Value, _NodeAlloc>`
// befriends `_Rb_tree`) and bits/stl_tree.h:1735/1775; <map>, <set>,
// <regex> (docs/status/libstdcxx.md, WP-366; regressed by PR #92).
// Verified against clang 19.
//@ expect: compile-ok
template<typename T> struct alloc {};
template<typename K, typename V> class handle {};
template<typename V> class handle<V, V> {
  template<typename K2, typename V2, typename A2> friend class tree;
  int secret = 7;
};
template<typename K, typename V, typename A = alloc<V> >
class tree {
public:
  tree() {}
  tree& operator=(const tree& x);
  tree& operator=(tree&& x) noexcept(sizeof(A) > 0);
  int peek(const handle<V, V>& h) const;
};
template<typename K, typename V, typename A>
tree<K, V, A>&
tree<K, V, A>::operator=(const tree&) { return *this; }
template<typename K, typename V, typename A>
inline tree<K, V, A>&
tree<K, V, A>::operator=(tree&&) noexcept(sizeof(A) > 0) { return *this; }
template<typename K, typename V, typename A>
int tree<K, V, A>::peek(const handle<V, V>& h) const { return h.secret; }
int main() {
  tree<int, int> a, b;
  a = b;
  a = static_cast<tree<int, int>&&>(b);
  handle<int, int> h;
  return a.peek(h) == 7 ? 0 : 1;
}
