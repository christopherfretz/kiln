// [temp.res.general]/1, [basic.lookup.qual.general]/1, [temp.inst]/2: in a
// non-template context, `Q::name` where the typedef `Q` names a not yet
// instantiated member class of a class template specialization (or the
// specialization itself) is looked up in that class once it is
// instantiated, and `name` is a type when it declares one. kiln's
// lookahead, which cannot instantiate, classifies `name` in the pattern
// as it does for `L<int>::P::name` (WP-288; libc++
// rand.dist.norm.lognormal/param_types.pass.cpp:26,
// `typedef param_type::distribution_type distribution_type;`).
//@ strict-diags
template <class T = double>
struct L {
  typedef T result_type;
  struct P {
    typedef L distribution_type;
    typedef int I;
    static const int v = 7;
    static int g() { return 3; }
  };
};
template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };

void block_scope() {
  typedef L<> D;
  typedef D::P P;
  typedef P::distribution_type distribution_type;
  static_assert(same<D, distribution_type>::value);
}

void member_class_first_use() {
  typedef L<long>::P Q;
  Q::I x = Q::v;
  typedef Q::I I;
  I y = Q::g();
  (void)x;
  (void)y;
}

void specialization_first_use() {
  typedef L<char> LL;
  LL::P::I x = LL::P::g();
  LL::result_type c = 'c';
  (void)x;
  (void)c;
}

typedef L<short>::P NP;
NP::I ns = NP::v;
static_assert(same<NP::distribution_type, L<short>>::value);
