// C++14 [dcl.init]/17.6.3, the rejected side of
// temp-copy-second-step-cv-destination.cpp: the copy of the `T` temporary
// into a `const T` object must still be viable: a copy constructor taking
// a non-const reference cannot bind the temporary (reported, as in clang,
// as a copy of a `B`).
//@ args: -std=c++11
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:9 error "no viable constructor copying variable of type 'B'"
//@ diag: 12:12 note "candidate constructor not viable: no known conversion from 'B' to 'int' for 1st argument"
//@ diag: 12:20 note "candidate constructor not viable: expects an lvalue for 1st argument"
struct B { B(int); B(B&); };
const B b = 1;
