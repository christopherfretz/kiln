// [over.match.class.deduct]/1.5 (P1816): aggregate deduction candidates.
//@ xfail(kiln): aggregate deduction (P1816) is deferred past WP-071b (M3 scope: constructors and guides only)
//@ strict-diags
template<class T, class U> struct Agg {
  T a;
  U b;
};
Agg g{1, 2.0};
static_assert(__is_same(decltype(g), Agg<int, double>));
