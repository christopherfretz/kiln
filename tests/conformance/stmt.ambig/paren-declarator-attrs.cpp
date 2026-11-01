// [dcl.attr.grammar]/5 via [stmt.ambig]: an attribute-specifier-seq after
// a parenthesized declarator-id is a declarator suffix (clang's
// TryParseBracketDeclarator says `[[` is ambiguous, so `T(n) [[...]]` is
// a declaration) and then "cannot appear here".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:19 error "an attribute list cannot appear here"
//@ diag: 17:22 error "an attribute list cannot appear here"
//@ diag: 18:10 error "an attribute list cannot appear here"
//@ diag: 21:11 error "an attribute list cannot appear here"
//@ diag: 22:12 error "an attribute list cannot appear here"
//@ diag: 23:21 error "an attribute list cannot appear here"
struct T { T(); T(int); explicit operator bool() const; };
// `T(n)` followed by an attribute-specifier-seq is a declarator: the
// attributes after a parenthesized declarator appertain to nothing.
void x26() { T(n) [[maybe_unused]]; }
void y3() { if (T(n) [[]] = 1) {} }
int (n1) [[]];
int n2 [[]];
int (n3)[3] [[]];
int (*n4) [[]];
int ((n5)) [[]];
void f() { int (n6) [[]] = 1; (void)n6; }
int (n7 [[]]);
