// [dcl.attr.grammar]/4 (GNU extension): a GNU attribute list stops at the
// first token that is not an attribute name, so a scoped name like
// `gsl::Owner` ends it at `::`; the missing `)` is "expected ')'" with no
// "to match this '('" note, as in clang's ParseGNUAttributes (WP-878,
// WP-891; spec/parse.md). Verified against clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:26 error "expected ')'"
//@ diag: 22:23 warning "unknown attribute 'gsl' ignored"
//@ diag: 23:26 error "expected ')'"
//@ diag: 23:23 warning "unknown attribute 'gsl' ignored"
//@ diag: 24:26 error "expected ')'"
//@ diag: 24:23 warning "unknown attribute 'gsl' ignored"
//@ diag: 25:26 error "expected ')'"
//@ diag: 25:23 warning "unknown attribute 'foo' ignored"
//@ diag: 26:26 error "expected ')'"
//@ diag: 26:23 warning "unknown attribute 'gsl' ignored"
//@ diag: 27:35 error "expected ')'"
//@ diag: 28:26 error "'noinline' attribute takes no arguments"
//@ diag: 29:32 error "expected ')'"

struct __attribute__((gsl::Owner)) A {};
struct __attribute__((gsl::Owner(int))) B {};
struct __attribute__((gsl::Pointer)) C {};
struct __attribute__((foo::bar)) D {};
struct __attribute__((gsl::Owner, packed)) E {};
void f1() __attribute__((noinline);
void f2() __attribute__((noinline(1)));
int v __attribute__((aligned(4) packed));
void f3();
