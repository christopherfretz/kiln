//@ expect: ast
// [class.union.anon]/1, [expr.ref]/2, /7, [temp.inst]: an instantiated
// access to a member of an anonymous union keeps the conversions of its
// object as the pattern has them: the lvalue-to-rvalue conversion of a
// pointer operand of `->`, the derived-to-base cast of an object whose
// anonymous member is a base's, and the materialization of a prvalue
// object (clang `TreeTransform::RebuildMemberExpr` for the unnamed
// member). The instantiation dropped the pointer's conversion, which
// tooling clients read and the constant evaluator crashed on (a
// bit-field assignment's truncation check folded `ct->a`). Reduced from
// LLVM 19 (`TypePropertyCache::ensure`).
struct T { union { unsigned a : 2; int other; }; };
struct D : T {};
T make();
template <class P> struct H {
  static void ptr(T *t, T *ct) { t->a = ct->a; }
  static void base(D *t, D &ct) { t->a = ct.a; }
  static unsigned prv() { return make().a; }
};
void use(T *t, D *d) {
  H<int>::ptr(t, t);
  H<int>::base(d, *d);
  H<int>::prv();
}
