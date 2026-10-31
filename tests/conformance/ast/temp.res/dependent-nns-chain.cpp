// [temp.dep.type]/6: a nested-name-specifier with a dependent prefix is
// rebuilt on substitution, staying a DependentNameType while its
// arguments are dependent and resolving once they are known.
//@ expect: ast
template <class T> struct M {
  template <class U> struct N {
    template <class V> struct O { using type = V; };
  };
};
template <class T, class U, class V>
using Deep = typename M<T>::template N<U>::template O<V>::type;
template <class T> struct Z { using type = Deep<T, T, long>; };
Z<int>::type z = 1;

template <class X> struct Q { using type = typename X::type::inner; };
struct In { struct type { using inner = int; }; };
template <class T> struct R { using t = typename Q<T>::type; };
R<In>::t r = 2;
