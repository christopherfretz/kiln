// [temp.param]/1, /17: a template parameter list has no varargs ellipsis,
// so a `...` after a non-type template parameter's declarator is always a
// misplaced pack ellipsis: "'...' must immediately precede declared
// identifier" for a named one, "'...' must be innermost component of
// anonymous pack declaration" for an abstract one (also after the pack's
// own `...`, `Ts......`). The parameter becomes a pack.
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 21:21 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 22:17 error "'...' must immediately precede declared identifier"
//@ diag: 23:18 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 23:21 error "expected ',' or '>' in template-parameter-list"
//@ diag: 24:18 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 25:18 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 26:17 error "'...' must immediately precede declared identifier"
//@ diag: 27:18 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 30:16 error "'...' must immediately precede declared identifier"
//@ diag: 31:20 error "'...' must be innermost component of anonymous pack declaration"
template <class... Ts> struct T {
  template <Ts (*)()...> struct A1;
  template <Ts a...> struct A2 {};
  template <Ts......a> struct A3;
  template <Ts[2]...> struct A4;
  template <Ts......> struct A5;
  template <Ts a..., int> struct A6;
  template <Ts......, int> struct A7;
  template <Ts *...> struct Ok;
};
template <int a...> struct B1 { static constexpr int n = sizeof...(a); };
template <int (*)()...> struct B2;
static_assert(B1<1, 2, 3>::n == 3);
T<int, char>::A2<1, 'a'> t;
