// [over.match.ctor]/1, [over.ics.rank]/3.3: siblings of
// conversion-function-beside-better-lvalue-cxx17-run.cpp that stay
// ambiguous, as the text reads: the copy constructor reached through the
// better 'operator N&' is deleted (GCC 16 selects it and rejects), and an
// rvalue conversion to a derived class with an object conversion as good
// as that of 'operator B&' (GCC 16: ambiguous). clang 22 lets the
// conversion function yielding the prvalue initialize the object
// directly. Found running GCC's C++ testsuite.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ xfail(clang): clang 22 accepts through its CWG2327 workaround; the text and GCC 16 reject
//@ diag: 25:5 error "call to constructor of 'N' is ambiguous"
//@ diag: 19:17 note "candidate constructor has been explicitly deleted"
//@ diag: 19:40 note "candidate constructor"
//@ diag: 26:5 error "call to constructor of 'B' is ambiguous"
//@ diag: 21:17 note "candidate constructor"
//@ diag: 21:31 note "candidate constructor"
struct N { N(); N(const N &) = delete; N(N &&); };
struct ToN { operator N() const; operator N &(); };
struct B { B(); B(const B &); B(B &&); };
struct D : B {};
struct ToB { operator B() const; operator B &(); operator D &&(); };
void f(ToN tn, ToB tb) {
  N n(tn);
  B b(tb);
}
