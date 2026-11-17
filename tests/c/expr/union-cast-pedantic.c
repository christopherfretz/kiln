// GNU cast to a union type: with -pedantic clang warns "cast to union type
// is a GNU extension" (-Wgnu-union-cast) at the `(`, also in a static
// initializer and under sizeof. An operand whose type, after its lvalue,
// array and function conversions, is not a member's unqualified type is an
// error naming that type (clang `err_typecheck_cast_to_union_no_type`):
// `short` against an `int` member, `char *` (a string literal) against a
// `const char *` one, `struct Q` against `struct P`, `int` against a union
// with only an unnamed bit-field of that type. WP-1587.
//@ std: c17
//@ args: -pedantic
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:20 warning "cast to union type is a GNU extension"
//@ diag: 25:23 warning "cast to union type is a GNU extension"
//@ diag: 27:15 warning "cast to union type is a GNU extension"
//@ diag: 28:15 error "cast to union type from type 'short' not present in union"
//@ diag: 29:15 error "cast to union type from type 'char *' not present in union"
//@ diag: 30:15 error "cast to union type from type 'struct Q' not present in union"
//@ diag: 31:15 error "cast to union type from type 'int' not present in union"
struct P { int a; };
struct Q { int a; };
union U { int i; const char *s; struct P p; };
union B { int : 4; long l; };
static union U g = (union U)1;
_Static_assert(sizeof((union U)1) == sizeof(union U), "");
void f(short sh, struct Q q, int i) {
  union U a = (union U)i;
  union U b = (union U)sh;
  union U c = (union U)"str";
  union U d = (union U)q;
  union B e = (union B)i;
}
