// [dcl.spec.auto.general]/12, [temp.inst]: a placeholder whose initializer
// is type-dependent stays undeduced until instantiation.
//@ strict-diags
template <class T> struct W {
  static T make();
  static constexpr auto size = sizeof(T);
  static auto get() { return make(); }
  static decltype(auto) ref() { static T t; return (t); }
  static int local() {
    auto v = make();
    return sizeof(v);
  }
};
static_assert(__is_same(decltype(W<int>::size), const unsigned long));
static_assert(W<double>::size == 8);
static_assert(__is_same(decltype(W<char>::get()), char));
static_assert(__is_same(decltype(W<long>::ref()), long &));
