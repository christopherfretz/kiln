// [dcl.init.aggr]/8, [class.dtor]/15: element destructor checks that clang
// reports twice at the same location (a member initialized from a
// temporary, the `}` of a brace-elided array, an implicit member of a
// nested aggregate); kiln reports each once (spec/diag.md §7.4), so the
// list is not strict-diags (WP-227).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ diag: 27:10 error "temporary of type 'D' has private destructor"
//@ diag: 20:11 note "implicitly declared private here"
//@ diag: 29:3 error "temporary of type 'D' has private destructor"
//@ diag: 20:11 note "implicitly declared private here"
//@ diag: 28:16 error "temporary of type 'D' has private destructor"
//@ diag: 20:11 note "implicitly declared private here"
//@ diag: 28:19 error "temporary of type 'D' has private destructor"
//@ diag: 20:11 note "implicitly declared private here"
//@ diag: 29:18 error "temporary of type 'D' has private destructor"
//@ diag: 20:11 note "implicitly declared private here"
//@ diag: 30:13 error "temporary of type 'D' has private destructor"
//@ diag: 20:11 note "implicitly declared private here"
class D { ~D(); public: D(); D(int); };
struct A { D d; int x; ~A(); };
struct B : D { int k; ~B(); };
struct H { D m[2]; ~H(); };
struct P { int a; D d; ~P(); };
struct Q { P p; int z; };
void f() {
  A b = {D(), 2};
  D n[2][2] = {1, 2
  }, o[1][2] = {{}};
  Q q = {1, 2, 3};
}
