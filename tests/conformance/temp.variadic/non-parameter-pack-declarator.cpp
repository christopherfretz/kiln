// [temp.variadic]/5, [dcl.decl.general]/4: only a function or template
// parameter declares a pack. A `...` before a variable, member or
// exception declarator-id is dropped with an error (the declaration's
// type then contains an unexpanded pack); before a ptr-operator or a
// parenthesized declarator it is misplaced too. [except.pre]/1: an
// exception-declaration may be abstract, so `catch (int(...))` names a
// function type (adjusted to a pointer, [except.handle]/2).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:4 error "only function and template parameters can be parameter packs"
//@ diag: 32:8 error "declaration type contains unexpanded parameter pack 'T'"
//@ diag: 33:5 error "declaration type contains unexpanded parameter pack 'T'"
//@ diag: 34:4 error "'...' must immediately precede declared identifier"
//@ diag: 34:4 error "only function and template parameters can be parameter packs"
//@ diag: 34:9 error "declaration type contains unexpanded parameter pack 'T'"
//@ diag: 35:11 error "only function and template parameters can be parameter packs"
//@ diag: 36:10 error "only function and template parameters can be parameter packs"
//@ diag: 38:15 error "only function and template parameters can be parameter packs"
//@ diag: 38:25 error "only function and template parameters can be parameter packs"
//@ diag: 39:5 error "only function and template parameters can be parameter packs"
//@ diag: 40:4 error "'...' must immediately precede declared identifier"
//@ diag: 40:4 error "only function and template parameters can be parameter packs"
//@ diag: 41:5 error "'...' must immediately precede declared identifier"
//@ diag: 41:5 error "only function and template parameters can be parameter packs"
//@ diag: 43:58 error "only function and template parameters can be parameter packs"
//@ diag: 43:55 warning "exception of type 'int' will be caught by earlier handler"
//@ diag: 43:37 note "for type 'int'"
//@ diag: 43:73 warning "exception of type 'int (*)(...)' will be caught by earlier handler"
//@ diag: 43:17 note "for type 'int (*)(...)'"
template<class... T> void g() {
  T... x;
  T y;
  T... *q;
  for (int... i = 0;;) {}
  if (int... c = 1) {}
}
struct S { int... m; int... f(); };
int ...y;
int... *p;
int ...(z);
void h() {
  try {} catch (int(...)) {} catch (int(x)) {} catch (int...) {} catch (int(*)(...)) {}
}
