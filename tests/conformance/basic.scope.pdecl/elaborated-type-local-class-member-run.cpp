// [basic.scope.pdecl]/7.2, [dcl.type.elab]/4: an elaborated-type-specifier
// `struct Q` that declares a new class in a local class's member function
// parameter list or member declaration declares it in the smallest
// enclosing block scope, that of the function (not of the class or the
// prototype), so the later `struct Q { ... };` in that block defines the
// same class, and calls passing a `Q *` type-check. The namespace-scope
// analogue declares it in the namespace; a function declaration in a
// block declares it in the block; a nested block's own `struct Q` is
// another class. Reduced from real code (WP-2506).
//@ expect: run
//@ stdout: 7 7 8 | 3 | 4 | 5 6
#include <cstdio>

struct NA {
  long g(struct NQ *q);
};
struct NQ { int m; };
long NA::g(NQ *q) { return q->m; }

int local() {
  struct A {
    A *p;
    long g(struct Q *q) { return q == pq ? 7 : 0; }
    struct Q *pq;
  };
  struct Q { long m; };
  struct B {
    long g(Q *q) { return q->m; }
    long h(struct Q *q) { return q->m + 1; }
  };
  A a;
  Q q{7};
  a.pq = &q;
  B b;
  std::printf("%ld %ld %ld | ", a.g(&q), b.g(&q), b.h(a.pq));
  return 0;
}

long block_fn(struct R *);
struct R { long m; };
long block_fn(R *r) { return r->m; }

int block() {
  long block_fn(struct R *);
  R r{4};
  return static_cast<int>(block_fn(&r));
}

int nested() {
  struct A { long g(struct S *s) const; };
  int inner = 0;
  {
    struct S { int z; };
    S s{5};
    inner = s.z;
  }
  struct S { long m; };
  S s{6};
  std::printf("%d %ld\n", inner, s.m);
  return 0;
}

int main() {
  local();
  NA na;
  NQ nq{3};
  std::printf("%ld | ", na.g(&nq));
  std::printf("%d | ", block());
  nested();
}
