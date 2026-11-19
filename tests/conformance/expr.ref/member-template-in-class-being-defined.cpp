// [expr.ref]/4, [expr.prim.this]/2: a member function template's trailing
// return type may look into its class although the class is still being
// defined, also when the template is substituted (deduced for a call)
// before the class's closing '}' and while the class is itself being
// instantiated. The SFINAE trait below must see `get_impl()` as valid;
// through an object of the base type the protected member stays
// inaccessible ([class.protected]/1). Reduced from real code (Boost.Beast's
// `is_fields_helper`).
//@ args: -std=c++17
template <class T> T&& declval();
template <bool B> struct bool_constant { static constexpr bool value = B; };
using true_type = bool_constant<true>;
using false_type = bool_constant<false>;

class F {
protected:
  int get_impl() const { return 0; }
};

template <class T>
struct helper : T {
  template <class U = helper>
  static auto f1(int) -> decltype(declval<int&>() = declval<U const&>().get_impl(), true_type());
  static auto f1(...) -> false_type;
  using t1 = decltype(f1(0));

  template <class U = T>
  static auto f2(int) -> decltype(declval<U const&>().get_impl(), true_type());
  static auto f2(...) -> false_type;
  using t2 = decltype(f2(0));
};
static_assert(helper<F>::t1::value, "protected member through the derived class");
static_assert(!helper<F>::t2::value, "protected member through the base class");

struct G : F {
  template <class U = G>
  static auto f1(int) -> decltype(declval<U const&>().get_impl(), true_type());
  static auto f1(...) -> false_type;
  using t1 = decltype(f1(0));
};
static_assert(G::t1::value, "a class being defined, not a template");

struct P {
  int q;
  template <class U> static auto f(int) -> decltype(declval<U const&>().q);
  template <class U> static void f(...);
  using t = decltype(f<P>(0));
  t *p;
};
static_assert(__is_same(decltype(P::p), int *), "a data member");
