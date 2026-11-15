// [dcl.type.elab]/2: an elaborated-type-specifier whose name is a typedef
// name is ill-formed ("typedef 'X' cannot be referenced with the 'struct'
// specifier"). clang declares an invalid class in its place, and nothing
// that uses the type is reported again: no "member access into incomplete
// type", "variable has incomplete type" or incomplete `sizeof`. Also after a
// class definition that conflicts with the typedef. WP-1435.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:8 error "definition of type 'state' conflicts with typedef of the same name"
//@ diag: 23:22 note "'state' declared here"
//@ diag: 25:17 error "typedef 'state' cannot be referenced with the 'struct' specifier"
//@ diag: 23:22 note "declared here"
//@ diag: 27:19 error "typedef 'anon' cannot be referenced with the 'struct' specifier"
//@ diag: 26:27 note "declared here"
//@ diag: 29:19 error "typedef 'I' cannot be referenced with the 'struct' specifier"
//@ diag: 28:13 note "declared here"
//@ diag: 30:23 error "typedef 'anon' cannot be referenced with the 'struct' specifier"
//@ diag: 26:27 note "declared here"
//@ diag: 31:26 error "typedef 'anon' cannot be referenced with the 'struct' specifier"
//@ diag: 26:27 note "declared here"
//@ diag: 32:8 error "typedef 'anon' cannot be referenced with the 'struct' specifier"
//@ diag: 26:27 note "declared here"
typedef enum { S_A } state;
struct state { int n; };
int take(struct state *p) { return p->n; }
typedef struct { int m; } anon;
int member(struct anon *x) { return x->m; }
typedef int I;
int scalar(struct I *x) { return x->m; }
void local() { struct anon v; v.m = 1; }
int size = sizeof(struct anon);
struct anon *get();
int call() { return get()->m; }
