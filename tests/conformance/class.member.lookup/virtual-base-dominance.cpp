// [class.member.lookup]/5-6: a member of a shared virtual base is found once,
// and a declaration in a class derived from the virtual base hides (dominates)
// the base's; with non-virtual bases the subobjects are distinct and ambiguous,
// except for static members.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:5 error "non-static member 'v' found in multiple base-class subobjects of type 'V':"
//@ diag: 11:16 note "member found by ambiguous name lookup"
//@ diag: 23:5 error "non-static member 'f' found in multiple base-class subobjects of type 'V':"
//@ diag: 11:24 note "member found by ambiguous name lookup"
struct V { int v; void f(); static int s; };
struct B1 : virtual V { void f(); };
struct B2 : virtual V {};
struct D : B1, B2 {};
struct N1 : V {};
struct N2 : V {};
struct E : N1, N2 {};
D d; E e;
void g() {
  d.f();
  d.v = 1;
  e.v = 2;
  e.f();
  (void)E::s;
}
