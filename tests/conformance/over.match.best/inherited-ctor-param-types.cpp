// [over.match.best.general]/2.7: a constructor of D is better than one
// inherited from a base B only when "for all arguments the corresponding
// parameters of F1 and F2 have the same type". With different parameter
// types and indistinguishable conversions the call is ambiguous, also
// through two levels of inheritance (two inherited constructors, the more
// derived class's is not preferred either). Found running GCC's C++
// testsuite; clang 19 and 22 apply the tie-break without the parameter
// condition and accept: the text and GCC 16 decide.
//@ xfail(clang): clang 19 and 22 ignore the same-parameter-types condition of [over.match.best.general]/2.7
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:3 error "call to constructor of 'D' is ambiguous"
//@ diag: 25:12 note "candidate inherited constructor"
//@ diag: 26:25 note "constructor from base class 'B' inherited here"
//@ diag: 26:28 note "candidate constructor"
//@ diag: 30:3 error "call to constructor of 'E' is ambiguous"
//@ diag: 25:12 note "candidate inherited constructor"
//@ diag: 27:25 note "constructor from base class 'D' inherited here"
//@ diag: 26:28 note "candidate inherited constructor"
//@ diag: 27:25 note "constructor from base class 'D' inherited here"
//@ diag: 31:3 error "call to constructor of 'D' is ambiguous"
//@ diag: 25:12 note "candidate inherited constructor"
//@ diag: 26:25 note "constructor from base class 'B' inherited here"
//@ diag: 26:28 note "candidate constructor"
struct B { B(int); };
struct D : B { using B::B; D(unsigned); };
struct E : D { using D::D; };
D d1(1L);
// Two inherited constructors, from D and from B: ambiguous too.
E e1(1L);
D d2 = {1L};
