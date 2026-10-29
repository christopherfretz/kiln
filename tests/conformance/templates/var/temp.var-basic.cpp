// [temp.pre]/1, [temp.inst]/1: a variable template declares a family of
// variables; each specialization is instantiated when first named.
template <class T> constexpr T zero = T(0);
template <class T> constexpr T pi = T(3.1415926535897932385L);
static_assert(zero<int> == 0);
static_assert(pi<int> == 3);
static_assert(pi<long> == 3);
static_assert(pi<double> > 3.14 && pi<double> < 3.15);
static_assert(&zero<int> == &zero<int>);
