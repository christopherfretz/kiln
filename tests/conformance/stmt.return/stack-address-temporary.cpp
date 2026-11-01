// [stmt.return]/6 (P2748R5), [class.temporary]/6: a returned reference
// bound, through a local reference's initializer, to the temporary that
// reference extended is ill-formed; clang notes each reference variable on
// the way. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:43 error "returning reference to local temporary object"
//@ diag: 16:29 note "binding reference variable 'r' here"
//@ diag: 17:63 error "returning reference to local temporary object"
//@ diag: 17:48 note "binding reference variable 'r2' here"
//@ diag: 17:30 note "binding reference variable 'r' here"
//@ diag: 18:45 warning "returning address of local temporary object"
//@ diag: 18:24 note "binding reference variable 'r' here"
struct B { int x; };
struct C { operator B() const { return B{1}; } };
const B& f1(C c) { const B& r = c; return r; }
const int& f2() { const int& r = 3; const int& r2 = r; return r2; }
int* f3() { const int& r = 3; return (int*)&r; }
const int& ok() { static const int& r = 3; return r; }
