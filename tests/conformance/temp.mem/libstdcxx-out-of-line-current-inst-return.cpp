// [temp.mem.func], [temp.over.link]/5, [temp.dep.type]/1: in an out-of-line
// member definition, `typename tree<K, V>::base_ptr` names the member of the
// current instantiation and is equivalent to `base_ptr` in the declaration.
// libstdc++ bits/stl_tree.h:2107 (_Rb_tree::_M_get_insert_unique_pos);
// <map>, <set>, 3 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): out-of-line definition with a current-instantiation qualified return type does not match
template<typename T1, typename T2> struct pair { T1 a; T2 b; };
template<typename K, typename V> class tree {
public:
  typedef int *base_ptr;
  typedef K key_type;
  pair<base_ptr, base_ptr> get_pos(const key_type &k);
};
template<typename K, typename V>
pair<typename tree<K, V>::base_ptr, typename tree<K, V>::base_ptr>
tree<K, V>::get_pos(const key_type &) { return {nullptr, nullptr}; }
int main() { tree<int, int> t; return t.get_pos(1).a != nullptr; }
