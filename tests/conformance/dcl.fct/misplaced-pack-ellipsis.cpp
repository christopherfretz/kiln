// [dcl.fct]/3, /20 (declarator-id and abstract-pack-declarator): a `...`
// after a parameter's declarator that contains an unexpanded pack is a
// misplaced pack ellipsis, not a varargs ellipsis: clang's "'...' must
// immediately precede declared identifier" for a named declarator,
// "'...' must be innermost component of anonymous pack declaration" for
// an abstract one (also after the pack's own `...` when more follows,
// `Ts......a`, `Ts......, int`). The parameter becomes the pack it was
// meant to be. Function parameters and member function declarations;
// in a lambda a misplaced `...` right before `)` stays the
// -Wambiguous-ellipsis varargs case.
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 34:40 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 35:36 error "'...' must immediately precede declared identifier"
//@ diag: 36:37 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 36:40 error "expected ')'"
//@ diag: 36:31 note "to match this '('"
//@ diag: 37:41 error "'...' must immediately precede declared identifier"
//@ diag: 38:37 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 39:36 error "'...' must immediately precede declared identifier"
//@ diag: 40:37 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 41:43 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 42:39 error "'...' must immediately precede declared identifier"
//@ diag: 43:36 error "'...' must immediately precede declared identifier"
//@ diag: 45:19 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 46:15 error "'...' must immediately precede declared identifier"
//@ diag: 47:16 error "'...' must be innermost component of anonymous pack declaration"
//@ diag: 47:19 error "expected ')'"
//@ diag: 47:10 note "to match this '('"
//@ diag: 48:16 error "'...' must immediately precede declared identifier"
//@ diag: 49:22 error "'...' must immediately precede declared identifier"
//@ diag: 52:19 error "'...' must immediately precede declared identifier"
template <class... Ts> void f1(Ts (*)()...);
template <class... Ts> void f2(Ts a...);
template <class... Ts> void f3(Ts......a);
template <class... Ts> void f4(Ts (*p)()...);
template <class... Ts> void f5(Ts[2]...);
template <class... Ts> void f6(Ts a..., int);
template <class... Ts> void f7(Ts......, int);
template <class... Ts> void f8(int (*)(Ts)...);
template <int... Ns> void f9(int a[Ns]...);
template <class... Ts> void fa(Ts a...) {}
template <class... Ts> struct S {
  void m1(Ts (*)()...);
  void m2(Ts a...);
  void m3(Ts......a);
  void m4(Ts *a...) {}
  friend void fr(Ts a...);
};
template <class... Ts> void l() {
  auto q = [](Ts a..., int) {};
}
void use() { fa<int, char>(1, 'a'); fa(1, 2.0, nullptr); }
// No misplaced pack: no unexpanded pack, or `...` right after the pack's
// own `...` and before `)`.
template <class... Ts> void ok1(int (*)(Ts...), ...);
template <class... Ts> void ok2(Ts *..., int);
