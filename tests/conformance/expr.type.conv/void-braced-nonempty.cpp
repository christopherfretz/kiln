// CWG2351 ([expr.type.conv]/2): only an empty braced-init-list makes a
// `void` prvalue; `void{1}` is ill-formed (clang "illegal initializer
// type 'void'" at the `{`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:16 error "illegal initializer type 'void'"
//@ diag: 9:27 error "illegal initializer type 'void'"
void f() { void{1}; }
void g(int i) { (void)void{i, i}; }
