// [class.member.lookup]/5-7, [class.mi]/4: a virtual base is one subobject
// however many paths reach it, a non-virtual one is a subobject per path.
// A member found in a shared virtual base and in a non-virtual base of the
// same type (F, M), or in two non-virtual subobjects (R's conversion), is
// ambiguous; found only through virtual bases (G, L3a) it is not. Each
// ambiguity lists one path per subobject, as clang does: kiln also listed
// `F -> C2 -> B -> A`, the virtual B already listed through C, until the
// member search visited each virtual base once (WP-3404).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:25 error "non-static member 'f' found in multiple base-class subobjects of type 'L0a':"
//@ diag: 25:19 note "member found by ambiguous name lookup"
//@ diag: 41:3 error "ambiguous conversion from derived class 'R' to base class 'A':"
//@ diag: 42:5 error "non-static member 'f' found in multiple base-class subobjects of type 'L0a':"
//@ diag: 25:19 note "member found by ambiguous name lookup"
//@ diag: 43:12 error "non-static member 'x' found in multiple base-class subobjects of type 'A':"
//@ diag: 18:16 note "member found by ambiguous name lookup"
struct A { int x; void f(); };
struct B : A {};
struct C : virtual B {};
struct C2 : virtual B {};
struct E : A {};
struct F : C, C2, E {};
struct G : C, C2 {};
struct L0a { void f(); };
struct L0b : virtual L0a {};
struct L1a : virtual L0a, virtual L0b {};
struct L1b : virtual L0a, virtual L0b {};
struct L2a : virtual L1a, virtual L1b {};
struct L2b : virtual L1a, virtual L1b {};
struct L3a : virtual L2a, virtual L2b {};
struct X : L0a {};
struct M : L3a, X {};
struct U : M { using M::f; };
struct P : B {};
struct Q : B {};
struct R : P, Q { using P::f; };
int g(F &f, G &gg, M &m, R &r) {
  gg.f();
  m.L3a::f();
  r.f();
  m.f();
  return f.x + gg.x;
}
