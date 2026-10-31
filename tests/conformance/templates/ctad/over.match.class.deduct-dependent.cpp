// [temp.dep.type], [over.match.class.deduct]: a placeholder initialized
// by type-dependent arguments is deduced when the template is
// instantiated.
//@ strict-diags
template<class T> struct H {
  T v;
  constexpr H(T x) : v(x) {}
};
template<class T> constexpr auto make_var(T t) {
  H h(t);
  return h;
}
template<class T> constexpr auto make_cast(T t) { return H(t); }
template<class T> constexpr auto make_list(T t) { return H{t}; }
template<class T> constexpr auto make_copy(T t) {
  H h = t;
  return h;
}
static_assert(__is_same(decltype(make_var(1.0)), H<double>));
static_assert(__is_same(decltype(make_cast('a')), H<char>));
static_assert(__is_same(decltype(make_list(1u)), H<unsigned>));
static_assert(__is_same(decltype(make_copy(2L)), H<long>));
static_assert(make_var(3).v + make_cast(4).v + make_list(5).v == 12);
// A non-dependent use inside a template is deduced at definition.
template<class T> int sized() {
  H h(1);
  static_assert(__is_same(decltype(h), H<int>));
  return sizeof(T);
}
int n = sized<char>();
