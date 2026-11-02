// [temp.mem.class], [temp.inst]/3, [temp.dep.type]/1: a member class of
// a specialization of a member class template, instantiated on its own,
// names a sibling member template of the outermost class template
// (`inner<C>`) and the enclosing specialization (`outer`). libc++
// lazy_split_view.h:190 `__outer_iterator<_Const>::value_type::begin`
// returning `__inner_iterator<_Const>` (WP-276).
//@ expect: run
//@ stdout: 7 1 3
#include <cstdio>
template <class V> struct view {
  template <bool> struct inner;
  template <bool C> struct outer {
    V v;
    struct value_type {
      outer i_;
      constexpr inner<C> begin() const { return inner<C>{i_}; }
      static constexpr int k = sizeof(view) + C;
    };
  };
  template <bool C> struct inner {
    outer<C> o;
    constexpr V get() const { return o.v; }
  };
  enum { three = 3 };
  template <bool C> struct other {
    struct deep { int f() { return three; } };
  };
};
static_assert(view<int>::outer<true>::value_type{{5}}.begin().get() == 5);
int main() {
  view<int>::outer<false>::value_type x{{7}};
  view<char>::other<true>::deep d;
  std::printf("%d %d %d\n", x.begin().get(), view<char>::outer<false>::value_type::k, d.f());
}
