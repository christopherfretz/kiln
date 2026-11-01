// [temp.deduct.guide]/1-3: deduction guides at namespace and class scope,
// template and non-template, explicit and with a requires-clause.
//@ strict-diags
template<class T> struct W { W(T); W(T *); };
W(const char *) -> W<long>;
template<class T> W(T *) -> W<T>;
template<class T> explicit W(T, T) -> W<T>;
template<class T>
  requires(sizeof(T) > 4)
W(T, int) -> W<T>;
namespace N {
template<class T> struct V { V(T); V(T *); };
template<class T> V(T *) -> V<T>;
} // namespace N
struct O {
  template<class T> struct I { I(T); I(T *); };
  I(int) -> I<long>;
  template<class T> I(T *) -> I<T>;
};
int i;
W w1(&i);
static_assert(__is_same(decltype(w1), W<int>));
N::V v(&i);
static_assert(__is_same(decltype(v), N::V<int>));
O::I o1(1);
static_assert(__is_same(decltype(o1), O::I<long>));
O::I o2(&i);
static_assert(__is_same(decltype(o2), O::I<int>));
