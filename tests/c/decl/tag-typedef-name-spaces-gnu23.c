// C17 6.2.3p1 (C23 6.2.3p1): tags (struct, union, enum) and ordinary
// identifiers (typedef names, objects, functions, enumeration constants)
// are separate name spaces, so a tag and a typedef spelled alike in one
// scope do not conflict, in either order, at file and block scope; the
// bare name finds the ordinary identifier and `struct X` the tag (clang
// and GCC accept; C++ [dcl.typedef]/6 does not, tests/conformance/
// dcl.typedef/tag-typedef-same-name-conflict.cpp). Found building an
// open-source HTTP client library. WP-1427.
//@ args: -std=gnu23 -pedantic-errors -Wall
//@ expect: compile-ok
//@ strict-diags
typedef enum { S_A, S_B } state;
struct state { int n; state s; };
int get(struct state *p) { return p->n + (int)p->s; }
_Static_assert(sizeof(state) == sizeof(int), "the bare name is the typedef");
_Static_assert(sizeof(struct state) == 2 * sizeof(int), "struct state is the tag");

struct t1 { char a; };
typedef long t1;
_Static_assert(sizeof(t1) == sizeof(long), "");
_Static_assert(sizeof(struct t1) == 1, "");

typedef int t2;
union t2 { char c[3]; };
_Static_assert(sizeof(union t2) == 3, "");
typedef short t3;
enum t3 { T3 };
enum t4 { T4A };
typedef double t4;
_Static_assert(sizeof(t4) == sizeof(double), "");

struct obj { int x; };
int obj = 3;
struct fn { int y; };
int fn(void) { return 1; }
enum e5 { e5 };
enum e5 e5v = e5;

typedef int fwd;
struct fwd;
struct fwd *pf;
struct fwd { fwd m; };

typedef struct same same;
struct same { int z; };
same sv = { 4 };

struct mem { int mem; int state; t1 t1; };

int blk(void) {
  typedef int b;
  struct b { b x; };
  struct b v = { 1 };
  b w = v.x;
  {
    struct b2 { int q; };
    typedef char b2;
    _Static_assert(sizeof(b2) == 1, "");
    _Static_assert(sizeof(struct b2) == sizeof(int), "");
    struct t1 { int inner; };
    typedef struct t1 t1;
    t1 it = { 5 };
    w += it.inner;
  }
  return w;
}

int use(void) {
  state s = S_B;
  t1 l = 1;
  struct t1 st = { 2 };
  struct mem m = { 1, 2, 3 };
  return (int)s + (int)l + st.a + obj + fn() + m.mem + m.state + (int)m.t1 + blk();
}
