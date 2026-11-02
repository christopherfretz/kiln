// [temp.param]/1, /17: a template parameter list has no varargs ellipsis,
// so a `...` after a non-type template parameter's declarator is always a
// misplaced pack ellipsis: "'...' must immediately precede declared
// identifier" for a named one, "'...' must be innermost component of
// anonymous pack declaration" for an abstract one (also after the pack's
// own `...`, `Ts......`). The parameter becomes a pack.
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 22:21 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 23:17 error "'...' must immediately precede declared identifier"
//@ diag: 24:18 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 24:21 error "expected ',' or '>' in template-parameter-list"
//@ diag: 25:18 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 26:18 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 27:17 error "'...' must immediately precede declared identifier"
//@ diag: 28:18 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 31:16 error "'...' must immediately precede declared identifier"
//@ diag: 32:20 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 34:16 error "'...' must immediately precede declared identifier"
template <class... Ts> struct T {
  template <Ts (*)()...> struct A1;
  template <Ts a...> struct A2 {};
  template <Ts......a> struct A3;
  template <Ts[2]...> struct A4;
  template <Ts......> struct A5;
  template <Ts a..., int> void A6();
  template <Ts......, int> void A7();
  template <Ts *...> struct Ok;
};
template <int a...> struct B1 { static constexpr int n = sizeof...(a); };
template <int (*)()...> struct B2;
static_assert(B1<1, 2, 3>::n == 3);
template <int a..., class U> constexpr int b3() { return sizeof...(a); }
