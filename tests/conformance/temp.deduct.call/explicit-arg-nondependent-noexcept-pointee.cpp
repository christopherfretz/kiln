// [temp.deduct.call]/4.2, [conv.fctptr]: with the explicit argument
// substituted, the pointee of `void (C::*)(T)` is the non-dependent
// `void (int)`; it still deduces against a `noexcept` member function
// (clang's `isSameOrCompatibleFunctionType` for a non-dependent P), and
// the function pointer conversion is checked on the deduced A: dropping
// `noexcept` is allowed, adding it (k2) is "deduced type ... does not
// match adjusted type". The shape of a downstream
// `extract_from<Side>(handle, &Handle::getSide)`
// over a `noexcept` getter (field-extraction header). Matches clang 21.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:45 error "no matching function for call to 'k2'"
//@ diag: 15:32 note "candidate template ignored: deduced type 'void (X::*)(int) noexcept' of 1st parameter does not match adjusted type 'void (X::*)(int)' of argument [with T = int, C = X]"
template<class T, class C> int k(void (C::*)(T)) { return 1; }
template<class T, class C> int k2(void (C::*)(T) noexcept) { return 2; }
struct X {
  void m(int) noexcept {}
  void nm(int) {}
};
int ok = k<int>(&X::m) + k<int>(&X::nm) + k2<int>(&X::m);
int bad() {
  return k<int, X>(&X::m) + k<int>(&X::m) + k2<int>(&X::nm);
}
