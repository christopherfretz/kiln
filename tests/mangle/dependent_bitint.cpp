// Mangling differential: `_BitInt(N)` and `unsigned _BitInt(N)` whose width
// is a template parameter (Itanium ABI <type> ::= DB <expression> _ and
// DU <expression> _). Both used to mangle as `u7unknown`, so the signed and
// unsigned overloads shared one symbol (fmt 12's `value(bitint<N>)` and
// `value(ubitint<N>)` constructors, WP-941).
template <int N> using bitint = _BitInt(N);
template <int N> using ubitint = unsigned _BitInt(N);
struct V {
  long long v;
  template <int N> V(bitint<N> x) : v(x) {}
  template <int N> V(ubitint<N> x) : v(x) {}
};
template <int N> void f(_BitInt(N + 1)) {}
template <class T, int N> void g(T, unsigned _BitInt(N), _BitInt(N)) {}

void use() {
  V a((_BitInt(15))1);
  V b((unsigned _BitInt(15))1);
  f<7>(0);
  g(1, (unsigned _BitInt(9))1, (_BitInt(9))1);
}
