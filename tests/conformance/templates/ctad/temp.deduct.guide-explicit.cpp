// [over.match.class.deduct]/3: in copy-initialization an explicit guide
// (user-declared, or from an explicit constructor) is not a candidate;
// in direct-initialization it is.
//@ strict-diags
template<class T> struct E { E(T); };
explicit E(int) -> E<long>;
E e1(1);
static_assert(__is_same(decltype(e1), E<long>));
E e2 = 1;
static_assert(__is_same(decltype(e2), E<int>));
E e3{1};
static_assert(__is_same(decltype(e3), E<long>));
template<class T> struct X {
  explicit X(T);
  X(T, int);
};
X x1(2.0);
static_assert(__is_same(decltype(x1), X<double>));
X x2 = {'a', 1};
static_assert(__is_same(decltype(x2), X<char>));
static_assert(__is_same(decltype(X(1u)), X<unsigned>));
