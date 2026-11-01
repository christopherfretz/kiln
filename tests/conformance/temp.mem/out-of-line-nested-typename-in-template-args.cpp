// [temp.dep.type]/1, [temp.mem.func]: in an out-of-line member definition
// the return type `pair<typename H<T>::iterator, bool>` names the
// current instantiation inside a template argument and must match the
// in-class `pair<iterator, bool>` (libc++ <__hash_table>:
// `pair<typename __hash_table<...>::iterator, bool> __hash_table<...>::
// __node_insert_unique(...)`).
//@ expect: run
//@ strict-diags
template <class A, class B> struct pair { int x = 0; };
template <class P> struct it {};
template <class T> struct H {
  typedef T* ptr;
  typedef it<ptr> iterator;
  pair<iterator, bool> f(int);
  iterator g(int);
  pair<iterator, iterator> h(int);
  iterator* k(int);
};
template <class T>
pair<typename H<T>::iterator, bool> H<T>::f(int i) { return {i}; }
template <class T>
typename H<T>::iterator H<T>::g(int) { return {}; }
template <class T>
pair<typename H<T>::iterator, typename H<T>::iterator> H<T>::h(int i) {
  return {i + 1};
}
template <class T>
typename H<T>::iterator* H<T>::k(int) { return nullptr; }
int main() {
  H<int> s;
  s.g(0);
  return s.f(2).x + s.h(3).x == 6 && !s.k(0) ? 0 : 1;
}
