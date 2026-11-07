// [class.member.lookup]/8: `Q::m` converts the object to its `Q`
// subobject, then to the member's class (clang
// `PerformObjectMemberConversion`: two derived-to-base steps), for
// fields and for the implicit object argument of a member call (WP-726).
struct impl { int c; int color() const { return c; } };
template <int> struct tramp : impl { int pad; };
struct n1 : tramp<0> { int q; };
struct n2 : n1, tramp<1> {
  int f() const { return tramp<1>::color() + n1::color(); }
  int g() { return tramp<1>::c + this->tramp<0>::c; }
};
int use(n2& x, n2* p) { return x.tramp<1>::c + p->n1::c + p->tramp<1>::color() + x.f() + x.g(); }
