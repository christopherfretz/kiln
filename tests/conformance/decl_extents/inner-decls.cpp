// The declaration-extent table (spec/ast.md §19.2) for declarations
// found missing or overwritten by the identifier-accounting test
// (§18.8): a range-based for statement's or an expansion statement's
// loop variable and its bindings ([stmt.ranged]/1, [stmt.expand]/2),
// up to its declarator, the bindings of a condition
// ([stmt.pre]/1, [dcl.struct.bind]), an exception declaration
// ([except.pre]/1), a requires-expression's parameters
// ([expr.prim.req.general]/2), init-captures, each a variable declared
// as if by `auto name initializer` ([expr.prim.lambda.capture]/6), and
// friend declarations naming a class declared before them or a member
// of another class ([class.friend]/2, /6): the earlier declaration keeps
// its row.
//@ decl-extents: inner-decls.extents
//@ std: c++26
struct P {
  int a, b;
  explicit operator bool() const { return a != 0; }
};
void f(P (&arr)[2], P p) {
  for (const P &x : arr)
    (void)x;
  for (auto [a, b] : arr)
    (void)a, (void)b;
  if (auto &[c, d] = p)
    (void)c, (void)d;
  template for (auto [g, h] : {p, p})
    (void)g, (void)h;
  try {
  } catch (const P &e) {
    (void)e;
  }
}
template <class T>
concept Addable = requires(T t, T u) { t + u; };
struct G;
struct H {
  void h(G &);
};
struct G {
  friend struct P;
  friend void H::h(G &);
};
int target = 0;
auto lam = [n = 1, &r = target] { return n + r; };
