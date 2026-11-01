// GNU vector attributes among the decl-specifiers apply to the
// decl-specifier type (clang processTypeAttrs, TAL_DeclSpec), so they
// make a function return a vector, a pointer point to one, and an array
// hold them (WP-156).
typedef short s4 __attribute__((vector_size(8)));
typedef int v4i __attribute__((vector_size(16)));

__attribute__((vector_size(8))) short g();
short __attribute__((vector_size(8))) g2();
__attribute__((vector_size(16))) int *p;
__attribute__((vector_size(16))) int (*fp)();
__attribute__((vector_size(16))) int a[2];
int x __attribute__((vector_size(16)));

struct S {
  __attribute__((vector_size(8))) short m();
  static __attribute__((vector_size(8))) short n();
};

void use(__attribute__((vector_size(8))) short f());

static_assert(__is_same(decltype(g()), s4));
static_assert(__is_same(decltype(g2()), s4));
static_assert(__is_same(decltype(S().m()), s4));
static_assert(__is_same(decltype(S::n()), s4));
static_assert(__is_same(decltype(*p), v4i&));
static_assert(__is_same(decltype(fp()), v4i));
static_assert(sizeof(a) == 32);
static_assert(sizeof(x) == 16);

s4 call() { return g() + g2(); }
