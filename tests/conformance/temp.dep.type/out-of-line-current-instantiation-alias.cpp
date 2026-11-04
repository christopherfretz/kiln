// [temp.dep.type]/1, [temp.alias]/2: an alias template specialization whose
// argument is a qualified name of the current instantiation
// (`ident_t<typename holder<T, C>::element_type>`) written before the
// declarator-id of an out-of-line member is the same type as the in-class
// `ident_t<element_type>`, in return and parameter positions and through
// `*`, `&` and `const` (WP-553).
//@ expect: compile-ok
//@ strict-diags

template <class T> struct ident { using type = T; };
template <class T> using ident_t = typename ident<T>::type;
template <class T> struct strip { using type = T; };
template <class T> struct strip<T[]> { using type = T; };
template <class T> using strip_t = typename strip<T>::type;

template <class T, class C> struct holder {
  using element_type = T;
  strip_t<element_type>* get() const noexcept;
  const ident_t<element_type>& ref(ident_t<element_type>& r) const;
  template <class U> strip_t<element_type> conv(U) const;
  void* value;
};
template <class T, class C>
strip_t<typename holder<T, C>::element_type>* holder<T, C>::get() const noexcept {
  return static_cast<strip_t<element_type>*>(value);
}
template <class T, class C>
const ident_t<typename holder<T, C>::element_type>& holder<T, C>::ref(
    ident_t<typename holder<T, C>::element_type>& r) const {
  return r;
}
template <class T, class C>
template <class U>
strip_t<typename holder<T, C>::element_type> holder<T, C>::conv(U u) const {
  return strip_t<element_type>(u);
}

int use() {
  int k = 2;
  holder<int[], char> a{&k};
  holder<int, char> b{&k};
  return *a.get() + a.conv(1L) + b.ref(k) + b.conv(1L);
}
