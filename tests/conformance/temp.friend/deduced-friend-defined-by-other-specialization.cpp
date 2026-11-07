// [temp.inst]/4, [dcl.spec.auto.general]/11-12: a friend function declared
// without a body in one class template specialization (`key<N>`) and
// defined, with a deduced return type, in a specialization of another
// class template (`value<T>`) is one function; once `value<T>` is
// instantiated, a call found through `key<N>`'s declaration instantiates
// that definition and deduces the return type. This is the stateful type
// map of std::execution implementations (`__typeid_lookup`): before WP-789
// kiln said "function 'lookup' with deduced return type cannot be used
// before it is defined".
//@ expect: run
//@ std: c++20
//@ stdout: 7 2.5
#include <cstdio>

struct idx {
  const int* p;
  constexpr bool operator==(const idx&) const = default;
};
template <class T> inline constexpr int tag_v = 0;

template <idx Id>
struct key {
  friend constexpr auto lookup(key<Id>) noexcept;
};

template <class T>
struct value {
  using type = T;
  static constexpr idx id = idx{&tag_v<T>};
  friend constexpr auto lookup(key<id>) noexcept { return value<T>(); }
};

template <auto Id>
extern decltype(lookup(key<Id>())) splice_v;
template <class T> inline constexpr idx typeid_of = value<T>::id;
template <idx Id> using splice = typename decltype(splice_v<Id>)::type;

static_assert(__is_same(int, splice<typeid_of<int>>));
static_assert(__is_same(double, splice<typeid_of<double>>));

// The same through an implicitly instantiated class with a plain int key.
template <int N> struct k2 { friend constexpr auto get(k2<N>) noexcept; };
template <class T, int N> struct v2 {
  friend constexpr auto get(k2<N>) noexcept { return T(7); }
};
v2<int, 1> a;
v2<double, 2> b;

int main() {
  splice<typeid_of<int>> i = get(k2<1>());
  decltype(get(k2<2>())) d = 2.5;
  std::printf("%d %g\n", i, d);
}
