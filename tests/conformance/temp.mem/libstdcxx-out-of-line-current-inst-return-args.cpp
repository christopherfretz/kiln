// [temp.mem.func], [temp.over.link]/5, [temp.dep.type]/1: a template
// argument `typename X<P...>::m` in the return type of an out-of-line
// member definition names the member of the current instantiation; when
// `m` is not dependent the specialization is the non-dependent one the
// in-class declaration named, else it stays dependent (companion of
// libstdcxx-out-of-line-current-inst-return.cpp, WP-323).
// Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
template<typename T1, typename T2> struct pair { T1 a; T2 b; };
template<typename T> struct alloc {};
template<typename T, typename A = alloc<T>> struct vec { T *p; };

template<typename K, typename V> class tree {
public:
  typedef int *base_ptr;
  typedef K key_type;
  base_ptr f1();
  pair<base_ptr, int> f2();
  pair<K, base_ptr> f3();
  vec<base_ptr> f4();
  vec<key_type> f5();
  const pair<base_ptr, base_ptr> *f6();
};
template<typename K, typename V>
typename tree<K, V>::base_ptr tree<K, V>::f1() { return nullptr; }
template<typename K, typename V>
pair<typename tree<K, V>::base_ptr, int> tree<K, V>::f2() { return {}; }
template<typename K, typename V>
pair<K, typename tree<K, V>::base_ptr> tree<K, V>::f3() { return {}; }
template<typename K, typename V>
vec<typename tree<K, V>::base_ptr> tree<K, V>::f4() { return {}; }
template<typename K, typename V>
vec<typename tree<K, V>::key_type> tree<K, V>::f5() { return {}; }
template<typename K, typename V>
const pair<typename tree<K, V>::base_ptr, typename tree<K, V>::base_ptr> *
tree<K, V>::f6() { return nullptr; }

int main() {
  tree<long, int> t;
  return (t.f1() != nullptr) + t.f2().b + (t.f3().b != nullptr) +
         (t.f4().p != nullptr) + (t.f5().p != nullptr) + (t.f6() != nullptr);
}
