// [namespace.udecl]/1, [class.member.lookup]/7: the name in a
// using-declarator is looked up as a qualified name; an ambiguous lookup set
// (a member of two subobjects of the same type, or members of different
// classes) makes the using-declaration ill-formed. kiln introduced the
// first declaration found without a diagnostic (WP-3404).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:25 error "non-static member 'f' found in multiple base-class subobjects of type 'A':"
//@ diag: 15:17 note "member found by ambiguous name lookup"
//@ diag: 20:25 error "non-static member 'x' found in multiple base-class subobjects of type 'A':"
//@ diag: 15:26 note "member found by ambiguous name lookup"
//@ diag: 22:47 error "member 'g' found in multiple base classes of different types"
//@ diag: 21:18 note "member found by ambiguous name lookup"
//@ diag: 21:43 note "member found by ambiguous name lookup"
struct A { void f(); int x; };
struct B : A {};
struct C : A {};
struct D : B, C {};
struct U : D { using D::f; };
struct V : D { using D::x; };
struct A2 { void g(); }; struct B2 { void g(int); };
struct Y : A2, B2 {}; struct Z : Y { using Y::g; };
