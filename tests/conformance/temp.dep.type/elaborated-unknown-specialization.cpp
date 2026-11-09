// [temp.dep.type]/6: 'struct A<T>::K' in another template names a member of
// an unknown specialization; the primary A has no K, but A<int> does, so
// nothing is looked up until instantiation (kiln rejected this before
// WP-1017 with "no struct named 'K' in 'A<T>'").
//@ std: c++20
//@ expect: run
template <class T> struct A {};
template <> struct A<int> { struct K { int v; }; };
template <class T> struct C {
  int f(struct A<T>::K *p) { return p->v; }
  friend struct A<T>::K;
};
int main() {
  A<int>::K k{3};
  return C<int>().f(&k) - 3;
}
