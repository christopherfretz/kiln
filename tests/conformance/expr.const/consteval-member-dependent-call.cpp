// A call of a consteval static member of a class template whose type is
// dependent (`_S_quote().substr(0, 1)` in libstdc++ 16 <format>
// `_Escapes<_CharT>::_S_term`; WP-525) is a dependent call in the pattern,
// never formed as an immediate invocation there. The reference to the
// member is not recorded for the "address of consteval function" check:
// the function is in a dependent context (clang `MarkDeclRefReferenced`,
// `!FD->isDependentContext()`), and the instantiated call is an immediate
// invocation ([expr.const]/16). Taking the address is still diagnosed at
// instantiation; an uninstantiated one is not. Verified against clang 19.
//@ expect: compile-fail
//@ diag: 26:33 error "cannot take address of consteval function 'f' outside of an immediate invocation"
//@ diag: 30:18 note "in instantiation of member function 'Q<int>::p' requested here"
//@ diag: 22:24 note "declared here"
template <class C> struct SV {
  const C* p;
  constexpr SV sub(int) const { return *this; }
  constexpr int n() const { return 1; }
};
template <class C> struct Q {
  static consteval SV<C> quote() { return SV<C>{nullptr}; }
  static constexpr SV<C> term(int t) { return t ? quote().sub(1) : quote(); }
  static consteval int f() { return 1; }
  static constexpr C value() { return C(f()); }
  static constexpr int length() { return (quote()).n(); }
  static int (*u())() { return &f; }
  static int (*p())() { return &f; }
};
static_assert(Q<char>::term(1).p == nullptr && Q<char>::value() == 1);
static_assert(Q<wchar_t>::length() == 1);
auto x = Q<int>::p();
