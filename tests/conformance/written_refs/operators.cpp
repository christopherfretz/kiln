// Operator expressions ([over.match.oper], [over.sub], [over.call],
// [over.inc], [over.ass]; spec/ast.md §18.13): the operator token of an
// operator expression whose overload resolution selected a function has
// an `operator` entry naming it, `how` saying whether it was a member
// candidate, found by argument-dependent lookup or by unqualified
// lookup. A built-in operator has none, also on class operands converted
// to a built-in type. `[]` and `()` record their closing token.
//@ written-refs: operators.refs
namespace lib {
struct V {
  int &operator[](int);
  int operator()(int, int) const;
  V &operator++();
  V operator++(int);
  V &operator=(int);
  V &operator+=(const V &);
  bool operator!() const;
  operator int() const;
};
V operator-(const V &);
V operator*(const V &, const V &);
V operator<<(const V &, int);
} // namespace lib

struct W {};
W operator+(W, W);
W operator~(W);
W &operator--(W &);

void use(lib::V v, lib::V u, W w, int *p) {
  v[1];
  v(1, 2);
  ++v;
  v++;
  v = 3;
  v += u;
  !v;
  -v;
  v * u;
  v << 2;
  w + w;
  ~w;
  --w;
  v + 1;
  p[0];
  *p;
  auto l = [](int x) { return x; };
  l(4);
  using lib::operator-;
  -u;
  v[v(1, 2)] = 0;
}
