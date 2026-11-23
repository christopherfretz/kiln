// C11 6.2.7/3 (composite type), [dcl.attr.grammar]/5 analogue: in C two
// declarations whose parameters differ in `noescape` are compatible, and
// the composite type's parameter is `noescape` only where both are
// (clang `mergeExtParameterInfo`); a definition's parameter is
// `nocapture` only from its own, and a call's argument from the
// declaration it names. spec/sema.md §4.5 "Parameter attributes",
// WP-2476.
//@ expect: ir
void h1(int *__attribute__((noescape)) p);
void h1(int *p) { *p = 1; }
void h2(int *p);
void h2(int *__attribute__((noescape)) p) { *p = 2; }
void h3(int *p);
void g3(int *x) { void h3(int *__attribute__((noescape)) p); h3(x); }
void g4(int *x) { h3(x); }
void h4(int *__attribute__((noescape)) p);
void g5(int *x) { h4(x); }
void h4(int *p);
void g6(int *x) { h4(x); }
