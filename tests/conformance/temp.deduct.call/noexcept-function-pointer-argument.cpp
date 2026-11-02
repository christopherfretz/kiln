// [temp.deduct.call]/4.2, [conv.fctptr]: deduction from a call argument
// ignores a non-deduced `noexcept` mismatch of a function type (clang's
// type match), and the deduced A is then checked against the argument:
// only a top-level function pointer conversion (or a qualification
// conversion) may differ. Adding `noexcept`, or a mismatch below a second
// pointer, is "deduced type ... does not match adjusted type". A pointer
// to a noexcept member function deduces against `void (C::*)(T)`.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:3 error "no matching function for call to 'f'"
//@ diag: 15:24 note "candidate template ignored: deduced type 'void (**)(int)' of 1st parameter does not match adjusted type 'void (**)(int) noexcept' of argument [with T = int]"
//@ diag: 26:3 error "no matching function for call to 'g'"
//@ diag: 16:24 note "candidate template ignored: deduced type 'void (*)(int) noexcept' of 1st parameter does not match adjusted type 'void (*)(int)' of argument [with T = int]"
template<class T> void f(void (**)(T));
template<class T> void g(void (*)(T) noexcept);
template<class T, class C> void k(void (C::*)(T));
template<class T> void q(void (*const *)(T));
struct X { void m(int) noexcept; };
void fn(int) noexcept;
void fn2(int);
void (*pfn)(int) noexcept;
void (*pfn2)(int);
void t() {
  f(&pfn);
  g(&fn2);
  k(&X::m);
  q(&pfn2);
  g(&fn);
}
