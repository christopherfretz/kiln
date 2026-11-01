//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:22 error "attempt to use a deleted function"
//@ diag: 17:13 note "'~D2' has been explicitly marked deleted here"
//@ diag: 23:20 error "temporary of type 'P' has private destructor"
//@ diag: 18:11 note "implicitly declared private here"
//@ diag: 25:21 error "attempt to use a deleted function"
//@ diag: 17:13 note "'~D2' has been explicitly marked deleted here"
//@ diag: 26:12 error "temporary of type 'P' has private destructor"
//@ diag: 18:11 note "implicitly declared private here"
//@ diag: 26:17 warning "expression result unused"
// [class.temporary]/4, [class.dtor]/15: a temporary's destructor must be
// accessible and not deleted even when it is trivial (clang
// `MaybeBindToTemporary` checks, then binds nothing). [dcl.type.decltype]/2:
// the call that is the operand of decltype makes no temporary, but a call
// nested in it does.
struct D2 { ~D2() = delete; };
class P { ~P() = default; public: P() = default; };
D2 mk();
struct Ok { int i; };

using T = decltype(D2{});
using U = decltype(P());
using V = decltype(mk());
using W = decltype((mk(), mk()));
void g() { P(); Ok{}; }
