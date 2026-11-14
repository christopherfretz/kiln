// [dcl.typedef]/6: in C++ a class or enumeration and a typedef of the
// same name in one scope conflict (unlike C, whose tags are a name space
// of their own, tests/c/decl/tag-typedef-name-spaces-*.c): the tag after
// the typedef is clang's "definition of type conflicts with typedef of the
// same name", for struct, union and enum; `typedef struct S S;` is fine.
// WP-1427.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:8 error "definition of type 'state' conflicts with typedef of the same name"
//@ diag: 17:27 note "'state' declared here"
//@ diag: 20:7 error "definition of type 't2' conflicts with typedef of the same name"
//@ diag: 19:13 note "'t2' declared here"
//@ diag: 22:6 error "definition of type 't3' conflicts with typedef of the same name"
//@ diag: 21:15 note "'t3' declared here"
//@ diag: 25:18 error "typedef 'state' cannot be referenced with the 'struct' specifier"
//@ diag: 17:27 note "declared here"
typedef enum { S_A, S_B } state;
struct state { int n; state s; };
typedef int t2;
union t2 { char c; };
typedef short t3;
enum t3 { T3 };
typedef struct same same;
struct same { int z; };
void take(struct state *p);
