// Fold-expression operators (spec/ast.md §18.13.6): each operator token of
// a fold-expression ([expr.prim.fold]/1) over an operand that could be of
// class or enumeration type is a DEPENDENT entry with the found set of the
// definition context; a binary fold has two tokens, two entries. An
// expansion ([temp.variadic]/11) applies the operator once per element
// pair: each specialization has one row per distinct resolution, so a
// heterogeneous pack gives a row per operator function and a
// `(builtin)` row for the built-in pairs ([over.built]). An expansion that
// applies no operator (an empty pack of `&&`, `||` or `,`,
// [temp.variadic]/12, or a single element) still reached the token and
// has a `(builtin)` row. A fold over a pack of `int` non-type template
// parameters cannot select an operator function and has no entry. A
// template never instantiated has the entries and no row.
//@ written-refs: operators-fold.refs
namespace lib {
struct S {
  int v;
};
S operator+(S, S);
S operator+(S, int);
bool operator&&(S, bool);
} // namespace lib

template <class... T> auto sum(T... t) { return (t + ...); }
template <class... T> auto lsum(T... t) { return (... + t); }
template <class... T> auto init(T... t) { return (0 + ... + t); }
template <class... T> bool all(T... t) { return (t && ...); }
template <int... N> int ints() { return (N + ... + 0); }
template <class... T> auto never(T... t) { return (t + ...); }

int run() {
  lib::S s{};
  sum(s, s, s);
  sum(s, 1);
  sum(1, 2);
  sum(s);
  lsum(s, s);
  init(1, 2);
  init();
  all();
  all(s, true);
  return ints<1, 2>();
}
