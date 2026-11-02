// [temp.dep.type]/8, [temp.over.link]/5: a template argument that is an
// alias of a dependent alias template specialization naming a concrete type
// (`rebind<T, int>` = `int*`) is not dependent, so `pair<nbp>` in the class
// and `pair<typename tree<T>::nbp>` before an out-of-line declarator-id name
// the same specialization; likewise through `*`, `&` and `&&` around the
// member typedef (libc++'s `__tree::__find_equal`).
//@ expect: compile-ok
//@ strict-diags

template <class A> struct pair { A a; };
template <class P, class U> using rebind = U*;

template <class T> struct tree {
  using nbp = rebind<T, int>;
  using ip = int*;
  pair<nbp> f1();
  pair<ip&> f2();
  pair<ip*> f3();
  pair<ip&&> f4();
  template <class K> pair<nbp&> f5(const K&);
  static_assert(__is_same(pair<nbp>, pair<int*>));
};

template <class T> pair<typename tree<T>::nbp> tree<T>::f1() { return {}; }
template <class T> pair<typename tree<T>::ip&> tree<T>::f2() {
  static int* p;
  return {p};
}
template <class T> pair<typename tree<T>::ip*> tree<T>::f3() { return {}; }
template <class T> pair<typename tree<T>::ip&&> tree<T>::f4() {
  static int* p;
  return {static_cast<int*&&>(p)};
}
template <class T>
template <class K>
pair<typename tree<T>::nbp&> tree<T>::f5(const K&) {
  static int* p;
  return {p};
}

int main() {
  tree<long> t;
  static_assert(__is_same(decltype(t.f1()), pair<int*>));
  static_assert(__is_same(decltype(t.f2()), pair<int*&>));
  static_assert(__is_same(decltype(t.f5(0)), pair<int*&>));
  (void)t.f1().a;
  (void)t.f3().a;
  (void)t.f5(1).a;
  return 0;
}
