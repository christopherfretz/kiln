// Operators in unevaluated operands and requires-expressions outside
// templates ([expr.context], [expr.prim.req]; spec/ast.md §18.13.2):
// overload resolution runs on them as on any other, so they have the
// same entries, their enclosing declaration the one around the
// requires-expression.
//@ std: c++20
//@ written-refs: operators-requires.refs
struct N {
  N operator*(N) const;
  bool operator<(N) const;
};
N operator-(N);
int operator""_u(unsigned long long);

constexpr bool ok = requires(N a, N b) {
  a * b;
  { a < b };
  -a;
  1_u;
};
using T = decltype(N{} * N{});
constexpr auto n = sizeof(-N{});
constexpr bool k = noexcept(N{} < N{});
