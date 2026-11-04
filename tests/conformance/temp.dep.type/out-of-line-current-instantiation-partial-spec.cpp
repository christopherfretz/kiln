// [temp.dep.type]/1.2, [temp.spec.partial.member]: inside the out-of-line
// definition of a member of a partial specialization, `X<A...>::` written
// with the partial specialization's own arguments (a non-dependent non-type
// argument `2` written unconverted, or a type pattern `T*`) names the
// current instantiation, so `typename X<A...>::m` is the member `m` and
// the definition matches the in-class declaration in parameter and return
// positions, through `const`, `&` and `*`, for member templates, static
// data members and members of a member class template of a non-template
// class (WP-553).
//@ expect: compile-ok
//@ strict-diags

template <int L, class T> struct vec;
template <class T> struct vec<2, T> {
  typedef int length_type;
  typedef T value_type;
  T& operator[](length_type i);
  const T& operator[](length_type i) const;
  length_type size() const;
  const value_type* data() const;
  template <class U> U as(const length_type& n) const;
  static length_type count;
  T x, y;
};
template <class T> T& vec<2, T>::operator[](typename vec<2, T>::length_type i) { return i ? y : x; }
template <class T> const T& vec<2, T>::operator[](typename vec<2, T>::length_type i) const {
  return i ? y : x;
}
template <class T> typename vec<2, T>::length_type vec<2, T>::size() const { return 2; }
template <class T> const typename vec<2, T>::value_type* vec<2, T>::data() const { return &x; }
template <class T>
template <class U>
U vec<2, T>::as(const typename vec<2, T>::length_type& n) const {
  return U(n);
}
template <class T> typename vec<2, T>::length_type vec<2, T>::count = 2;

template <class T> struct box;
template <class T> struct box<T*> {
  using elem = T;
  elem& get();
  void put(const elem&);
  T* p;
};
template <class T> typename box<T*>::elem& box<T*>::get() { return *p; }
template <class T> void box<T*>::put(const typename box<T*>::elem& e) { *p = e; }

struct outer {
  template <int N, class T> struct part;
};
template <class T> struct outer::part<1, T> {
  typedef long L;
  L f(L) const;
  static const L* q;
};
template <class T> typename outer::part<1, T>::L outer::part<1, T>::f(typename outer::part<1, T>::L v) const {
  return v;
}
template <class T> const typename outer::part<1, T>::L* outer::part<1, T>::q = nullptr;

int use() {
  vec<2, int> v{1, 2};
  const vec<2, int>& cv = v;
  int k = 3;
  box<int*> b{&k};
  b.put(4);
  outer::part<1, int> p;
  return v[0] + cv[1] + v.size() + *cv.data() + v.as<int>(1) + vec<2, int>::count + b.get() +
         int(p.f(5)) + (outer::part<1, int>::q == nullptr);
}
