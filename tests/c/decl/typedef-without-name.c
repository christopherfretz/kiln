// C23 6.7p2 (declarations): a declaration other than a static_assert
// declares a declarator, a tag or an enumeration's members. Outside a
// structure an unnamed structure or union is no anonymous member (C23
// 6.7.2.1p15), so it declares nothing; `typedef` with no declarator is
// clang's "typedef requires a name" (-Wmissing-declarations).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 15:1 warning "typedef requires a name"
//@ diag: 16:1 warning "typedef requires a name"
//@ diag: 17:1 warning "typedef requires a name"
//@ diag: 18:1 warning "declaration does not declare anything"
//@ diag: 19:1 warning "declaration does not declare anything"
//@ diag: 21:15 warning "typedef requires a name"
//@ diag: 21:36 warning "declaration does not declare anything"
typedef union { int a; };
typedef struct S { int b; };
typedef int;
union { int c; };
static struct { int d; };
struct T { union { int x; }; struct { int y; }; } t;
int f(void) { typedef enum { E0 }; union { int z; }; return t.x + t.y; }
