// The join of the table with the expressions (spec/ast.md §18.11.1):
// the location kiln stores on a DeclRefExpr or MemberExpr built from an
// id-expression or member name is the name's entry's `name_lo` (its
// `range.lo`, or the `~` of a destructor name), one entry per
// expression, naming the same declaration. One of each accessor case:
// a qualified id and a globally qualified one, a template-id, an
// operator-function-id (qualified, and after `.`), a
// conversion-function-id, destructor names after `.` and `->` with and
// without a qualifier, and a member template after `->template`.
// written_refs_accounting.rs checks the join over this directory.
//@ written-refs: join.refs
//@ std: c++20
namespace n {
struct V {
  int x;
  ~V() {}
  bool operator==(const V &) const = default;
  operator int() const { return x; }
  template <class T> T as() const { return T(x); }
};
bool operator<(V a, V b) { return a.x < b.x; }
int f(int v) { return v; }
template <class T> T id(T v) { return v; }
} // namespace n
int use(n::V &a, n::V *p) {
  int r = n::f(1) + ::n::f(2) + n::id<int>(3) + n::id(4);
  r += n::operator<(a, *p) + a.operator==(*p) + a.operator int();
  r += p->template as<int>() + a.template as<long>() + p->x;
  a.~V();
  p->n::V::~V();
  return r;
}
// A template-id resolved early and again against its target
// ([over.over]/1): the entry names the specialization the target
// deduces, as the expression does.
template <int N> void wrap(void *) {}
template <int N, class T0> void wrap(void *, T0) {}
using WF = void(void *, int);
WF *retarget() { return wrap<3>; }
