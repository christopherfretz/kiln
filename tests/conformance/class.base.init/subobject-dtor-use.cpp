// [class.base.init]/12: every non-delegating constructor definition
// potentially invokes the destructor of each base and member subobject,
// with or without written mem-initializers; a delegating constructor
// potentially invokes the class's own destructor (clang
// `MarkBaseAndMemberDestructorsReferenced` / `SetDelegatingInitializer`).
// A trivial destructor that is non-public or deleted is still checked
// (clang `hasIrrelevantDestructor`).
//@ expect: compile-fail
//@ diag: 31:17 error "attempt to use a deleted function"
//@ diag: 28:12 note "'~D' has been explicitly marked deleted here"
//@ diag: 32:21 error "attempt to use a deleted function"
//@ diag: 28:12 note "'~D' has been explicitly marked deleted here"
//@ diag: 33:17 error "attempt to use a deleted function"
//@ diag: 28:12 note "'~D' has been explicitly marked deleted here"
//@ diag: 33:35 error "attempt to use a deleted function"
//@ diag: 33:13 note "destructor of 'A3' is implicitly deleted because base class 'D' has a deleted destructor"
//@ diag: 28:12 note "'~D' has been explicitly marked deleted here"
//@ diag: 34:13 error "base class 'P' has private destructor"
//@ diag: 29:11 note "implicitly declared private here"
//@ diag: 36:15 error "field of type 'Q' has protected destructor"
//@ diag: 30:23 note "declared protected here"
//@ diag: 37:15 error "field of type 'P' has private destructor"
//@ diag: 29:11 note "implicitly declared private here"
//@ diag: 44:26 error "attempt to use a deleted function"
//@ diag: 28:12 note "'~D' has been explicitly marked deleted here"
//@ diag: 45:19 error "attempt to use a deleted function"
//@ diag: 28:12 note "'~D' has been explicitly marked deleted here"
struct D { ~D() = delete; };
class P { ~P() = default; friend struct F1; };
struct Q { protected: ~Q() = default; };
struct A1 : D { A1() {} };
struct A2 { D d[2]; A2(int) {} };
struct A3 : D { A3(int) {} A3() : A3(1) {} };
struct A4 : P { A4() {} };
struct A5 : Q { A5() {} };
struct A6 { Q q; A6() {} };
struct A7 { P p; A7(); };
A7::A7() {}
template<class T> struct A8 : T { A8() {} };
A8<Q> a8;
A8<D> *a8p;
union U { D d; U() {} };
struct A9 { union { D d; }; A9() {} };
struct A10 : virtual D { A10() {} };
struct A11 { D d; ~A11() {} };
