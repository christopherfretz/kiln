// [over.match.class.deduct]/1.3, [over.match.best.general]/2.11: the copy
// deduction candidate `C(C<T>) -> C<T>` makes `C x(c)` copy rather than
// wrap, and beats an equally good constructor guide.
//@ strict-diags
template<class T> struct C {
  T v;
  constexpr C(T x) : v(x) {}
};
constexpr C c1(1);
C c2(c1);
static_assert(__is_same(decltype(c2), C<int>));
C c3 = c1;
static_assert(__is_same(decltype(c3), C<int>));
C c4{c1};
static_assert(__is_same(decltype(c4), C<int>));
static_assert(__is_same(decltype(C(c1)), C<int>));
// A user-declared copy constructor's guide ties with the copy candidate.
template<class T> struct K {
  K(T);
  K(const K &);
};
K k1(1);
K k2(k1);
static_assert(__is_same(decltype(k2), K<int>));
constexpr C c5(c1);
static_assert(c5.v == 1);
