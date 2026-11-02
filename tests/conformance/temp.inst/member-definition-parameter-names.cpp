// [temp.inst]/4, [basic.scope.block]/2: a member function of a class
// template is defined from its out-of-class definition, whose parameter
// names are the ones its body sees; a local named like the in-class
// declaration's parameter (`p`) is no redefinition of it (libc++
// `forward_list::erase_after(const_iterator __f)` declared with `__p`).
// Packs and constructors as well. Verified against clang 19.
//@ expect: run
template <class T> struct L {
  int f(T p);
  template <class U> int g(U p, T q);
  template <class... Ts> int h(Ts... p);
  L(T p);
  T v;
};
template <class T> int L<T>::f(T q) {
  T p = q + 1;
  return p;
}
template <class T> template <class U> int L<T>::g(U a, T b) {
  T p = a + b;
  int q = 0;
  return p + q;
}
template <class T> template <class... Ts> int L<T>::h(Ts... xs) {
  int p = (0 + ... + xs);
  return p;
}
template <class T> L<T>::L(T w) : v(w) {
  T p = w;
  v = p;
}
int main() {
  L<int> l(1);
  return l.f(1) == 2 && l.g(2, 3) == 5 && l.h(1, 2, 3) == 6 && l.v == 1 ? 0 : 1;
}
