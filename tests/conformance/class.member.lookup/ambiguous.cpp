// [class.member.lookup]/5-7: a name found in more than one base subobject is
// ambiguous unless every declaration is a static member, type or enumerator
// of the same class; names in a derived class hide those of its bases.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(kiln): needs qualified member access (d.L::x): act_on_member takes no nested-name-specifier
//@ diag: 21:20 error "member 'x' found in multiple base classes of different types"
//@ diag: 16:20 note "member found by ambiguous name lookup"
//@ diag: 17:20 note "member found by ambiguous name lookup"
//@ diag: 22:20 error "non-static member 'a' found in multiple base-class subobjects of type 'V':"
//@ diag: 15:16 note "member found by ambiguous name lookup"
//@ diag: 24:25 error "member 'x' found in multiple base classes of different types"
//@ diag: 16:20 note "member found by ambiguous name lookup"
//@ diag: 17:20 note "member found by ambiguous name lookup"
struct V { int a; static int s; enum { e }; };
struct L : V { int x; };
struct R : V { int x; int y; };
struct D : L, R {
  int y;
  int f() { return y + s + e; }
  int g() { return x; }
  int h() { return a; }
};
int use(D d) { return d.x + d.L::x + d.y + D::s; }
