// [temp.explicit]/12, [dcl.fct.def.default]/5: under an explicit
// instantiation declaration, a member explicitly defaulted on its first
// declaration is not instantiated but implicitly defined where it is
// odr-used, so it is emitted linkonce_odr as clang does; the other members,
// including one defaulted out of class, stay declarations. Reduced from
// libstdc++'s `extern template class __shared_ptr<filesystem::_Dir>`, whose
// library does not export `__shared_ptr(const __shared_ptr&) = default`.
//@ args: -std=c++23
struct N {
  N();
  N(const N &);
  ~N();
  N &operator=(const N &);
  bool operator==(const N &) const;
};
template <class T> struct S {
  N n;
  S() = default;
  S(int) {}
  S(const S &) = default;
  S(S &&);
  S &operator=(const S &) = default;
  ~S() = default;
  bool operator==(const S &) const = default;
  void f() {}
};
template <class T> S<T>::S(S &&) = default;
extern template struct S<int>;
bool use() {
  S<int> a, b(1);
  S<int> c(a), d(static_cast<S<int> &&>(b));
  c = d;
  a.f();
  return a == c;
}
