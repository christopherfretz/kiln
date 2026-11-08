// GNU `cleanup(f)` on a local variable (AttrDocs "cleanup"; WP-807): `f(&v)`
// runs when `v` leaves its scope, along every exit (fallthrough, `return`,
// `break`, `continue`, `goto`, unwinding), in reverse declaration order and
// before `v`'s own destructor. The parameter may be `void *` or a more
// qualified pointer. A template's local keeps it in each instantiation; a
// VLA has it too. (A `bool` parameter: clang passes the address
// unconverted, kiln `true`; cleanup-accepted.cpp only compiles it.)
//@ expect: run
//@ stdout: F[a2 a1 ] R[r] B0 b B1 b C0 c C1 c G[g] E[e caught] O[x ~x y ~y ] T[5 7 ] V[3 ] ]
extern "C" int printf(const char*, ...);

void out(const char **p) { printf("%s ", *p); }
void outv(void *p) { printf("%s ", *static_cast<const char **>(p)); }
void outc(const char *const *p) { printf("%s ", *p); }
void vo(void *p) { printf("%d ", *static_cast<int *>(p)); }

#define CL(f) __attribute__((cleanup(f)))

void fallthrough() {
  printf("F[");
  const char *a1 CL(out) = "a1";
  const char *a2 CL(out) = "a2";
}

int ret() {
  printf("R[");
  const char *r CL(outv) = "r]";
  return 1;
}

void loops() {
  for (int i = 0; i < 3; ++i) {
    const char *b CL(out) = "b";
    printf("B%d ", i);
    if (i == 1) break;
  }
  for (int i = 0; i < 2; ++i) {
    const char *c CL(outc) = "c";
    printf("C%d ", i);
    if (i == 0) continue;
  }
}

void go() {
  printf("G[");
  {
    const char *g CL(out) = "g]";
    goto done;
  }
done:;
}

void thrower() {
  const char *e CL(out) = "e";
  throw 1;
}

void eh() {
  printf("E[");
  try {
    thrower();
  } catch (int) {
    printf("caught] ");
  }
}

struct D {
  const char *n;
  ~D() { printf("~%s ", n); }
};
void dout(D *d) { printf("%s ", d->n); }

void order() {
  printf("O[");
  D y CL(dout) = {"y"};
  D x CL(dout) = {"x"};
}

void ti(int *p) { printf("%d ", *p); }
template <int N> void tmpl() {
  int v CL(ti) = N;
}

void vla(int n) {
  printf("V[");
  const char *s CL(out) = "]";
  int a[n] CL(vo);
  a[0] = 3;
}


int main() {
  fallthrough();
  printf("] ");
  ret();
  loops();
  go();
  eh();
  order();
  printf("] T[");
  tmpl<5>();
  tmpl<7>();
  printf("] ");
  vla(2);
  printf("]\n");
}
