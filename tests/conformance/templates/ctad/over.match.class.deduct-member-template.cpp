// [temp.mem], [over.match.class.deduct]/1: a member class template of an
// instantiated class template, named through an alias of the
// specialization, takes its guides from the original member's
// constructors with the enclosing arguments substituted; its member
// deduction guides are redeclared with it.
//@ strict-diags
template<class T> struct Outer {
  template<class U> struct In { In(T, U); };
  template<class... Us> struct P { P(T, Us...); };
  template<class U> struct D { D(T *, U); };
  template<class U> struct G { G(U, int); };
  template<class U> struct H { template<class V> H(V, T); };
  template<class U> H(U *, T) -> H<U>;
};
using O = Outer<int>;
O::In b(1, 2.0);
static_assert(__is_same(decltype(b), Outer<int>::In<double>));
O::P p(1, 'c', 2L);
static_assert(__is_same(decltype(p), Outer<int>::P<char, long>));
int x;
O::D d(&x, 'c');
static_assert(__is_same(decltype(d), Outer<int>::D<char>));
O::In c = b;
static_assert(__is_same(decltype(c), Outer<int>::In<double>));
using OC = Outer<char>;
OC::G g(1.0f, 2);
static_assert(__is_same(decltype(g), Outer<char>::G<float>));
char cc;
O::H h(&cc, 1);
static_assert(__is_same(decltype(h), Outer<int>::H<char>));
