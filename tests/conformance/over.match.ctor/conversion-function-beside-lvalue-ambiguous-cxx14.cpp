// [over.match.ctor]/1 with C++14 [dcl.init]/17.6.2: before C++17 no
// conversion function initializes the object directly, so a copy
// constructor reached through 'operator B&' and a move constructor reached
// through 'operator B() const' are indistinguishable ([over.ics.rank]/3.3)
// and the call is ambiguous (clang 22; GCC 16 binds 'B&&' through
// 'operator B&' too and calls the copy constructor). The C++17 sibling is
// conversion-function-beside-better-lvalue-cxx17-run.cpp. Found running
// GCC's C++ testsuite.
//@ std: c++14
//@ expect: compile-fail
//@ strict-diags
//@ xfail(gcc): GCC 16 does not set 'operator B&' aside for the move constructor's 'B&&' ([over.match.ref]/1.1)
//@ diag: 18:17 error "call to constructor of 'B' is ambiguous"
//@ diag: 16:17 note "candidate constructor"
//@ diag: 16:31 note "candidate constructor"
struct B { B(); B(const B &); B(B &&); };
struct U { operator B() const; operator B &(); };
void f(U u) { B b(u); }
