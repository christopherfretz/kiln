// [temp.friend]/1, [except.spec]/5, [temp.over.link]/5: a friend function
// template of a class template whose noexcept-specifier names its own
// template parameter redeclares a namespace-scope function template; its
// specifier, instantiated with the class, names the redeclared head's
// parameter (one level shallower) and is equivalent to an identical one
// written at namespace scope, before or after the class. A different
// operand (`N + 1`) is a mismatch, also against a later declaration.
// Reduced from g++.dg cpp0x/noexcept38.C.
//@ expect: compile-fail
//@ oracle: gcc
//@ skip(clang): clang keeps the friend's own parameter at its pattern depth, so even equivalent specifiers mismatch
//@ strict-diags
//@ diag: 23:31 error "exception specification in declaration does not match previous declaration"
//@ diag: 26:8 note "in instantiation of template class 'C<int>' requested here"
//@ diag: 19:22 note "previous declaration is here"
//@ diag: 28:22 error "exception specification in declaration does not match previous declaration"
//@ diag: 24:31 note "previous declaration is here"
template <int N> int ok1() noexcept(N);
template <int N> int bad1() noexcept(N + 1);
template <typename> struct C {
  template <int N> friend int ok1() noexcept(N);
  template <int N> friend int ok2() noexcept(N);
  template <int N> friend int bad1() noexcept(N);
  template <int N> friend int bad2() noexcept(N);
};
C<int> c;
template <int N> int ok2() noexcept(N);
template <int N> int bad2() noexcept(N + 1);
