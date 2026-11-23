// The declaration-extent table (spec/ast.md §19.2, §19.3) for the
// declarations a function body's statements make: a range-based for
// statement's loop variable and its bindings ([stmt.ranged]), an
// exception declaration ([except.pre]), an expansion statement's loop
// variable ([stmt.expand]), a requires-expression's parameters
// ([expr.prim.req.general]), and a label ([stmt.label]): its row runs
// from its attribute-specifier-seq, or else its identifier, to the `:`
// (and the GNU attributes after it), a label used before it is defined
// included. A GNU local label declaration (`__label__ l;`) is a
// declaration of the label too: the label then has two rows, the
// `__label__` declaration's and its definition's (§19.1; WP-2630).
//@ decl-extents: stmt-decls.extents
//@ std: c++26
struct B {
  int u, v;
};
void f(int (&a)[2], B (&b)[1]) {
  for (int x : a) {}
  for (auto [u, v] : b) {}
  try {} catch (int e) {}
  template for (auto y : {1, 2}) {}
}
template <class T>
concept C = requires (T t) { t; };
void l() {
  goto fwd;
fwd:
  goto l1;
l1:
  [[maybe_unused]] l2:
  l3: __attribute__((unused));
  {
    __label__ q, q2;
    q: goto q;
    q2:;
  }
}
