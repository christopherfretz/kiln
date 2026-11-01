// [dcl.init.aggr]/8, [class.dtor]/15: aggregate initialization
// potentially invokes the destructor of each element of class type, so
// it must be accessible: clang's checkDestructorReference at the clause
// (a member, before its initialization; a base, after it), at the `}`
// for an array element or an uninitialized member, and for an object a
// converting constructor creates ([dcl.init.general]/16.6.3). An array
// variable has no destructor check of its own (WP-227). kiln reports a
// diagnostic repeated at the same location once (spec/diag.md §7.4).
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 69:10 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 70:10 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 71:10 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 72:10 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 73:10 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 73:12 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 73:11 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 74:15 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 75:20 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 75:16 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 75:19 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 76:5 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 77:18 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 77:17 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 77:19 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 79:3 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 78:16 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 78:19 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 79:18 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 80:13 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 81:9 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 81:5 error "variable of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 82:15 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 82:10 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
//@ diag: 82:24 error "temporary of type 'D' has private destructor"
//@ diag: 62:11 note "implicitly declared private here"
class D { ~D(); public: D(); D(int); };
struct A { D d; int x; ~A(); };
struct B : D { int k; ~B(); };
struct H { D m[2]; ~H(); };
struct P { int a; D d; ~P(); };
struct Q { P p; int z; };
void f() {
  A a = {};
  A b = {D(), 2};
  B c = {{}, 1};
  B e = {};
  H h = {{1}};
  D arr[2] = {};
  D arr2[2] = {1, 2};
  D arr3[2];
  D m[2][2] = {{1}};
  D n[2][2] = {1, 2
  }, o[1][2] = {{}};
  Q q = {1, 2, 3};
  D v = 1;
  A g = {.d = 1, .x = 2};
}
