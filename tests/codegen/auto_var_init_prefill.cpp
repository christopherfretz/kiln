// [basic.indet]/1 (C++26, P2795R5) under -ftrivial-auto-var-init=zero
// (spec/codegen.md §7y): clang's `EmitAutoVarInit` writes the fill before
// any initialization that may leave bytes unwritten -- a non-trivial
// default constructor (and arrays of such classes), a non-constant
// initializer of a class, array, vector, atomic or reference variable,
// and a scalar whose initializer names the variable itself. A scalar
// with any other initializer, and a constant aggregate initializer, get
// no fill; the constant's padding is spelled as zero bytes.
//@ args: -ftrivial-auto-var-init=zero
void use(void *);
struct NT {
  int a, b;
  NT() : a(1) {}
  NT(int x) : a(x) {}
};
struct S {
  int a, b;
};
struct Pd {
  char c;
  int i;
};
union U {
  int i;
  double d;
};
S make();
int g();
int gi;
typedef float v4f __attribute__((vector_size(16)));

void ctor() {
  NT x;
  NT arr[3];
  NT n(3);
  use(&x);
  use(arr);
  use(&n);
}

void nonconstant(int y) {
  S s = make();
  int a[3] = {y, y, y};
  S t{g(), 2};
  auto [p, q] = make();
  use(&s);
  use(a);
  use(&t);
  use(&p);
}

void references(int y) {
  int &r = y;
  int &k = gi;
  use(&r);
  use(&k);
}

void scalars() {
  int x = x + 1;
  int i = g(), j = i + 1;
  _Atomic(int) at = g();
  use(&x);
  use(&j);
  use(&at);
}

void constants() {
  S s{1, 2};
  Pd p{1, 2};
  U u{1};
  S z{};
  use(&s);
  use(&p);
  use(&u);
  use(&z);
}

void vla(int n, int m) {
  int a[n];
  char c[n];
  int b[n][m];
  int d[n][3];
  use(a);
  use(c);
  use(b);
  use(d);
}

void unmarked() {
  [[clang::uninitialized]] int x;
  __attribute__((uninitialized)) S s;
  use(&x);
  use(&s);
}
