// Debug info (WP-910): a member typedef of a class template named by a
// non-dependent type in an instantiated member keeps the template
// pattern's typedef. Its scope is the unit, not the (dependent) pattern
// class, as in clang's `getContextDescriptor` ([temp.dep.type]).
//@ args: -g
template <class T, int N> struct Traits { using Type = T[N]; };
template <class T> struct Traits<T, 0> { struct Type {}; };
template <class T, int N> struct Arr {
  typedef unsigned long size_type;
  typename Traits<T, N>::Type elems;
  size_type size() const { return N; }
};
Arr<int, 3> a3;
Arr<int, 0> a0;
unsigned long use() { return a3.size() + a0.size(); }
