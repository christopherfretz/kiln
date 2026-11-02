// [temp.inst]/3, [basic.lookup.qual.general]/1, [dcl.type.simple]: a
// typedef naming a member class of a class template specialization that
// is not instantiated yet, used before `::` (`typedef L<>::P pt;
// pt::dt x;`), names that member class; the parser classifies `dt` by
// the member class's pattern and the member class is instantiated.
// libc++ `lognormal_distribution<>::param_type::distribution_type`
// (rand.dist.norm.lognormal/param_types.pass.cpp). WP-296.
//@ expect: compile-ok

template <class T = double> struct L {
  struct P {
    typedef L distribution_type;
    template <class U> using pair = U[2];
    static constexpr int v = sizeof(T);
    enum E { e = 3 };
  };
  struct Q;
};
template <class T> struct L<T>::Q {
  typedef T value_type;
};

int main() {
  typedef L<> D;
  typedef D::P param_type;
  typedef param_type::distribution_type distribution_type;
  static_assert(__is_same(D, distribution_type));
  param_type::pair<int> two = {1, 2};
  static_assert(param_type::v == sizeof(double) && param_type::e == 3);
  using q = L<char>::Q;
  q::value_type c = 'c';
  return two[0] + c - 'c' - 1;
}

typedef L<long>::P lp;
lp::distribution_type at_namespace_scope;
static_assert(__is_same(decltype(at_namespace_scope), L<long>));
