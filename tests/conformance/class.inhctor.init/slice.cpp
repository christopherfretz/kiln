// [over.match.funcs.general]/8: an inherited constructor of C whose first
// parameter is a reference to P is excluded for a single argument when the
// constructed class D is reference-related to P and P to C, whatever the
// argument (the excluded B(B &&, char) is not listed even though it would
// fail for an lvalue anyway); a constructor template specialization so
// excluded is listed as ignored (it is never a copy/move constructor).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:3 error "no matching constructor for initialization of 'D'"
//@ diag: 23:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'B' to 'const D' for 1st argument"
//@ diag: 23:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'B' to 'D' for 1st argument"
//@ diag: 22:49 note "candidate template ignored: inherited constructor cannot be used to copy object"
//@ diag: 23:25 note "constructor from base class 'B' inherited here"
//@ diag: 23:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
//@ diag: 28:3 error "no matching constructor for initialization of 'D'"
//@ diag: 23:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'B' to 'const D' for 1st argument"
//@ diag: 23:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'B' to 'D' for 1st argument"
//@ diag: 22:49 note "candidate template ignored: inherited constructor cannot be used to move object"
//@ diag: 23:25 note "constructor from base class 'B' inherited here"
//@ diag: 23:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
struct D;
struct B { B(); B(const D &); template<class T> B(T &&, int = 0); B(B &&, char = 0); };
struct D : B { using B::B; };
D d;
D d3(static_cast<D &&>(d));
B b;
D d4(b);
D d5(static_cast<B &&>(b));
