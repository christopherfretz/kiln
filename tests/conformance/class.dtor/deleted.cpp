// [class.dtor]/7: a defaulted destructor is deleted for a subobject with a
// deleted or inaccessible destructor and a variant member with a non-trivial one;
// [class.dtor]/15: a variable's destructor is potentially invoked, so it must be
// accessible and not deleted (extern declarations and pointers do not invoke it).
// Pinned to C++23 (P3074R7 changes the union rules in C++26: class.union/trivial-union-deleted-cxx26.cpp).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:4 error "attempt to use a deleted function"
//@ diag: 29:13 note "'~DD' has been explicitly marked deleted here"
//@ diag: 31:4 error "attempt to use a deleted function"
//@ diag: 29:13 note "'~DD' has been explicitly marked deleted here"
//@ diag: 34:4 error "variable of type 'PD' has private destructor"
//@ diag: 33:22 note "declared private here"
//@ diag: 35:4 error "variable of type 'QD' has protected destructor"
//@ diag: 33:55 note "declared protected here"
//@ diag: 38:3 warning "explicitly defaulted destructor is implicitly deleted"
//@ diag: 37:6 note "destructor of 'H' is implicitly deleted because field 'd' has a deleted destructor"
//@ diag: 29:13 note "'~DD' has been explicitly marked deleted here"
//@ diag: 38:10 note "replace 'default' with 'delete'"
//@ diag: 44:3 warning "explicitly defaulted destructor is implicitly deleted"
//@ diag: 42:5 note "destructor of 'UU' is implicitly deleted because variant field 'u' has a non-trivial destructor"
//@ diag: 44:11 note "replace 'default' with 'delete'"
//@ diag: 47:3 warning "explicitly defaulted destructor is implicitly deleted"
//@ diag: 46:12 note "destructor of 'B' is implicitly deleted because base class 'PD' has an inaccessible destructor"
//@ diag: 47:10 note "replace 'default' with 'delete'"
//@ diag: 50:28 error "attempt to use a deleted function"
//@ diag: 29:13 note "'~DD' has been explicitly marked deleted here"
struct DD { ~DD() = delete; };
DD dd;
DD arr[2];
extern DD e;
struct PD { private: ~PD(); }; struct QD { protected: ~QD(); };
PD pd;
QD qd;
struct H {
  DD d;
  ~H() = default;
};
struct U { ~U(); };
union UU {
  U u;
  int i;
  ~UU() = default;
};
struct B : PD {
  ~B() = default;
};
struct I { DD d; };
void f() { I *p; static DD s; }
