// [class.mem.general]/1: a virt-specifier-seq is part of a member-declarator
// only. After a function declarator outside a member-specification (an
// out-of-class member definition, a namespace- or block-scope function, a
// function template) each `final`/`override` is diagnosed and skipped
// (clang's "'override' specifier is not allowed outside a class
// definition"), and the declaration or definition goes on (WP-288).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:13 error "'override' specifier is not allowed outside a class definition"
//@ diag: 22:13 error "'final' specifier is not allowed outside a class definition"
//@ diag: 23:13 error "'final' specifier is not allowed outside a class definition"
//@ diag: 23:19 error "'override' specifier is not allowed outside a class definition"
//@ diag: 25:10 error "'override' specifier is not allowed outside a class definition"
//@ diag: 26:10 error "'final' specifier is not allowed outside a class definition"
//@ diag: 27:29 error "'override' specifier is not allowed outside a class definition"
//@ diag: 27:38 error "'final' specifier is not allowed outside a class definition"
//@ diag: 28:21 error "'override' specifier is not allowed outside a class definition"

struct B { virtual void f(); virtual void g(); virtual void h(); };
struct D : B { void f() override; void g() final; void h() override; };
void D::f() override {}
void D::g() final {}
void D::h() final override {}

void x() override;
void y() final;
template <class T> void t() override final;
void u() { void v() override; }

// Still identifiers elsewhere.
int final = 1;
int override(int);
int k = override(final);
