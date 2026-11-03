//@ args: -O1
// GNU `may_alias` at -O1 ([basic.lval]/11; clang `TypeHasMayAlias`): an
// access through a typedef-name carrying the attribute (also through a
// typedef of it, an alias-declaration, a pointer typedef) or through a
// may-alias class or enumeration is tagged `omnipotent char`; a member of
// a may-alias class is a may-alias access, while a may-alias class used
// as a member type keeps its struct node, and a may-alias member type in
// a struct path is the char access type at its offset. `!tbaa.struct` of
// a may-alias class copy lists char entries. `optnone` keeps the bodies.
#define ON __attribute__((optnone))
typedef int __attribute__((may_alias)) mint;
typedef mint mint2;
using aint [[gnu::may_alias]] = int;
typedef int* __attribute__((may_alias)) mptr;
struct __attribute__((may_alias)) MS { int a; float b; };
struct P { int x; mint y; MS m; };
enum __attribute__((may_alias)) ME { E0 };
union U { int i; };
ON int scalars(mint* p, mint2* q, aint* r, mptr* s, ME* e) {
  *p = 1; *q = 2; *r = 3; *s = 0; *e = E0;
  return *p + *q + *r + **s;
}
ON int fields(MS* m, P* p) {
  m->a = 1; m->b = 2; p->x = 3; p->y = 4; p->m.a = 5;
  return m->a + p->y + p->m.a;
}
ON void copies(MS* a, MS* b, P* c, P* d) { *a = *b; *c = *d; }
ON int arr(mint (*a)[4]) { return (*a)[1]; }
