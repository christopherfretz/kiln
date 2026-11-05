// [class.virtual]/4-5, [class.pre]/3 (C++11, N3272 / N3206): `override`
// and `final` virt-specifiers, `final` classes. WP-646 feature probe.
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 15:30 error "non-virtual member function marked 'override' hides virtual member function"
//@ diag: 12:67 note "hidden overloaded virtual function 'B::h' declared here: type mismatch at 1st parameter ('int' vs 'long')"
//@ diag: 16:13 error "base 'D' is marked 'final'"
//@ diag: 13:8 note "'D' declared here"
//@ diag: 17:22 error "declaration of 'g' overrides a 'final' function"
//@ diag: 12:43 note "overridden virtual function is here"
struct B { virtual void f(); virtual void g() final; virtual void h(int); };
struct D final : B { void f() override; void h(int) override final; };
int final = 1, override = 2;
struct D2 : B { void h(long) override; };
struct D3 : D {};
struct D4 : B { void g(); };
