// [dcl.init.aggr]/8, [class.dtor]/15: aggregate initialization
// potentially invokes the destructor of each element of class type, so
// it must be accessible: clang's checkDestructorReference at the clause
// (a member, before its initialization; a base, after it), at the `}`
// for an array element or an uninitialized member, and for an object a
// converting constructor creates ([dcl.init.general]/16.6.3). An array
// variable has no destructor check of its own (WP-227). The cases clang
// reports twice at one location are in element-dtor-access-repeat.cpp.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 57:10 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 58:10 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 59:10 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 60:10 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 60:12 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 60:11 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 61:15 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 62:20 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 62:16 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 62:19 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 63:5 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 64:18 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 64:17 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 64:19 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 65:9 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 65:5 error "variable of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 66:15 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 66:10 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
//@ diag: 66:24 error "temporary of type 'D' has private destructor"
//@ diag: 50:11 note "implicitly declared private here"
class D { ~D(); public: D(); D(int); };
struct A { D d; int x; ~A(); };
struct B : D { int k; ~B(); };
struct H { D m[2]; ~H(); };
struct P { int a; D d; ~P(); };
struct Q { P p; int z; };
void f() {
  A a = {};
  B c = {{}, 1};
  B e = {};
  H h = {{1}};
  D arr[2] = {};
  D arr2[2] = {1, 2};
  D arr3[2];
  D m[2][2] = {{1}};
  D v = 1;
  A g = {.d = 1, .x = 2};
}
