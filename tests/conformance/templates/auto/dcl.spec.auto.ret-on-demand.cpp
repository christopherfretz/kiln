// [dcl.spec.auto.general]/11, [temp.inst]/4-5: a function with a deduced
// return type used before its definition is instantiated (libc++'s
// `move_iterator::__get_iter_concept`) has its definition instantiated
// on demand when the return type is needed; definitions that are never
// needed are not instantiated (`bad`, `bad2`, `badf` would be ill-formed).
//@ expect: run
//@ exit: 0
struct RA {};
struct IN {};
template <class T> concept big = sizeof(T) >= 4;
template <class I> class mi {
  static constexpr auto get_concept() {
    if constexpr (big<I>) return RA{}; else return IN{};
  }
public:
  using concept_t = decltype(get_concept());
  static constexpr auto size() { return sizeof(I); }
  static constexpr decltype(auto) ref(I& i) { return (i); }
  using size_type = decltype(size());
};
template <class T> struct holder { using C = typename mi<T>::concept_t; };
template <class T> auto later(T);
template <class T> struct uses { using R = decltype(later(T())); };
template <class T> auto later(T t) { return t * 2L; }
template <class T> struct Outer {
  struct In { static auto f() { return T(); } };
  using F = decltype(In::f());
};
template <class T> struct S {
  auto a() { return b() + 1; }
  auto b() { return T(1); }
  static constexpr auto c() { return d(); }
  static constexpr auto d() { return sizeof(T); }
  decltype(auto) e() { return f(); }
  auto f() -> T { return 2; }
  template <class U> static auto g(U u) { return h(u); }
  template <class U> static auto h(U u) { return u; }
  auto bad() { return T::nope; }
  static auto bad2() { return T::nope2; }
};
template <class T> struct V : S<T> {
  using B = decltype(S<T>::c());
  auto k() { return this->a(); }
};
template <class T> auto badf(T) { return T::nope3; }
template <class T> using R = decltype(badf(T()));
template <class T> concept Has = requires { S<T>::c(); };
static_assert(__is_same(mi<int>::concept_t, RA));
static_assert(__is_same(holder<char>::C, IN));
static_assert(__is_same(mi<char>::size_type, decltype(sizeof(0))));
static_assert(__is_same(uses<int>::R, long));
static_assert(__is_same(Outer<short>::F, short));
static_assert(__is_same(V<int>::B, decltype(sizeof(0))));
static_assert(Has<int>);
static_assert(S<char>::c() == 1);
int x;
static_assert(__is_same(decltype(mi<int>::ref(x)), int&));
int main() {
  S<int> s;
  V<long> v;
  return s.a() + s.e() + S<int>::g(1) + (int)v.k() - 7;
}
