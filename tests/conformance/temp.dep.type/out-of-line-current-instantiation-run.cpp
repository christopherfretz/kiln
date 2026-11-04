// [temp.dep.type]/1, [temp.spec.partial.member], [temp.alias]/2: out-of-line
// members whose types name the current instantiation through
// `typename X<A...>::m` (a partial specialization's own arguments, an alias
// template, a nested class of a member class template) are the members the
// class declared and run as such (WP-553).
//@ expect: run
//@ stdout: 2 1 7 9
#include <cstdio>
#include <type_traits>

template <int L, class T> struct vec;
template <class T> struct vec<2, T> {
  typedef int length_type;
  T& operator[](length_type i);
  length_type size() const;
  T x, y;
};
template <class T> T& vec<2, T>::operator[](typename vec<2, T>::length_type i) { return i ? y : x; }
template <class T> typename vec<2, T>::length_type vec<2, T>::size() const { return 2; }

template <class T, class C> struct base {
  using element_type = T;
  std::remove_extent_t<element_type>* get() const noexcept;
  void* value;
};
template <class T, class C>
std::remove_extent_t<typename base<T, C>::element_type>* base<T, C>::get() const noexcept {
  return static_cast<std::remove_extent_t<element_type>*>(value);
}

struct policy {
  template <class HP> struct apply {
    struct type {
      struct owner { int v; };
      static owner noone;
    };
  };
};
template <class HP> typename policy::apply<HP>::type::owner policy::apply<HP>::type::noone = {7};

int main() {
  vec<2, int> v{0, 0};
  v[1] = 5;
  v[0] = v[1] + 4;
  int k[1] = {1};
  base<int[], int> b{k};
  std::printf("%d %d %d %d\n", v.size(), *b.get(), policy::apply<int>::type::noone.v, v[0]);
}
