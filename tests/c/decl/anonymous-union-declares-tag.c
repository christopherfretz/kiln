// C11 6.7.2.1/13: an anonymous union's members are members of the
// enclosing structure; a structure or enumeration tag declared inside it
// is an ordinary declaration of the enclosing scope (6.2.1/4, 6.7.2.3), so
// C accepts it where C++ rejects a type declared in an anonymous union
// ([class.union.anon]/1; clang checks C++ only). The tag and the
// enumerator are usable after the structure.
//@ std: c11
//@ expect: compile-ok
//@ strict-diags
struct S {
  union {
    int a;
    struct N { int b; } n;
    enum E { q = 3 } e;
    struct { int c; };
  };
};
struct N n2 = { q };
enum E e2 = q;
int f(struct S *s) { return s->a + s->n.b + s->c + n2.b + e2; }
