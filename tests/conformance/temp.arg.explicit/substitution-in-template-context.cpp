// [temp.arg.explicit]/2, [temp.deduct.general]/2, [temp.dep.candidate],
// [temp.res.general]/1: the explicit template arguments are substituted
// into the function parameter types (and into the types of later non-type
// template parameters) with the template's own scope current, so a name
// its declaration looks up again (`mk<T>()`, a dependent call) is found
// in the template's namespace or class, not from the scope of the call
// (`n::h<int>()` outside `n`).
//@ expect: compile-ok
//@ strict-diags
namespace n {
template <class T> T mk();
template <class T> struct id { using type = T; };

template <class T> constexpr int h(decltype(mk<T>()) * = nullptr) { return 1; }
template <class T>
constexpr int h2(typename id<decltype(mk<T>())>::type * = nullptr) { return 2; }
template <class T, decltype(mk<T>()) N> constexpr int nt() { return N; }
template <class T, class... U>
constexpr int pk(decltype(mk<T>()) *, U...) { return sizeof...(U); }

template <class T, decltype(mk<T>()) N> struct S { static constexpr int v = N; };
template <class T, decltype(mk<T>()) N> constexpr int vt = N;

struct C {
  static int sm();
  using member_t = long;
  template <class T> static constexpr int h(decltype(sm() + T()) * = nullptr) { return 3; }
  template <class T> static constexpr int h2(member_t *, T * = nullptr) { return 4; }
  template <class T> constexpr int h3(decltype(sm() + T()) * = nullptr) const { return 5; }
};

template <class X> struct W {
  static X make();
  using type = X;
  template <class T> static constexpr int f(decltype(make() + T()) * = nullptr) { return 6; }
  template <class T> static constexpr int g(typename W::type *, T * = nullptr) { return 7; }
};

namespace inner {
template <class T> constexpr int h(decltype(mk<T>()) * = nullptr) { return 8; }
}
} // namespace n

static_assert(n::h<int>() == 1);
static_assert(n::h2<int>() == 2);
static_assert(n::nt<int, 3>() == 3);
static_assert(n::pk<int, long>(nullptr, 1L) == 1);
static_assert(n::S<int, 4>::v == 4);
static_assert(n::vt<int, 5> == 5);
static_assert(n::C::h<int>() == 3);
static_assert(n::C::h2<int>(nullptr) == 4);
static_assert(n::C().h3<int>() == 5);
static_assert(n::W<short>::f<int>() == 6);
static_assert(n::W<short>::g<int>(nullptr) == 7);
static_assert(n::inner::h<char>() == 8);

