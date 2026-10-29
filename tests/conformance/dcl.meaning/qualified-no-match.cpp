// [dcl.meaning.general]/2: a qualified declarator-id must name a member
// previously declared in the nominated namespace (or one of its inline
// namespaces) or class.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:12 error "out-of-line definition of 'k' does not match any declaration in namespace 'N::M'"
//@ diag: 23:12 error "out-of-line definition of 'i2' does not match any declaration in namespace 'N::I'"
//@ diag: 24:8 error "out-of-line definition of 'h' does not match any declaration in namespace 'N'"
//@ diag: 25:9 error "out-of-line definition of 'n' does not match any declaration in 'S'"
//@ diag: 26:13 error "out-of-line definition of 'n' does not match any declaration in 'S::In'"
namespace N {
void f();
namespace M { void j(); }
inline namespace I { void i(); }
}
struct S { void m(); struct In {}; };

void N::f() {}
void N::M::j() {}
void N::i() {}
void S::m() {}
void N::M::k() {}
void N::I::i2() {}
int N::h() { return 0; }
void S::n() {}
void S::In::n() {}
