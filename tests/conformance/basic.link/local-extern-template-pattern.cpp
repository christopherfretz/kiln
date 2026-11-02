// [basic.link]/6, [temp.spec.general]/2, [temp.res.general]/6: a block-scope
// `extern` declaration in a function template pattern does not itself
// redeclare the entity; each instantiation does. So the pattern's
// `extern T e` is never the previous declaration of an instantiation (no
// "'int' vs 'T'"), a namespace-scope declaration after the pattern does not
// conflict with it, and a dependent type is checked only at instantiation;
// the pattern's own non-dependent `extern int g2` still conflicts with an
// earlier namespace-scope declaration. Two instantiations that disagree on
// the type do conflict (clang's `MergeVarDeclTypes`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:42 error "redeclaration of 'g2' with a different type: 'int' vs 'char'"
//@ diag: 29:13 note "previous declaration is here"
//@ diag: 34:42 error "redeclaration of 'g4' with a different type: 'int' vs 'long'"
//@ diag: 33:24 note "previous declaration is here"
//@ diag: 20:12 error "redeclaration of 'e' with a different type: 'char' vs 'int'"
//@ diag: 27:24 note "in instantiation of function template specialization 'f<char>' requested here"
//@ diag: 20:12 note "previous declaration is here"
template <class T> void f() {
  extern T e;
  extern int g;
}
template <class T> T h() {
  extern T e2;
  return e2;
}
void use() { f<int>(); f<char>(); h<long>(); }

extern char g2;
template <class T> void p() { extern int g2; extern T g3; }
extern char g3;
template <class T> void q() { extern T g5; extern int g5; }
void k() { extern long g4; }
template <class T> void m() { extern int g4; }
