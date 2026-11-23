// [basic.link]/4, [basic.link]/6, [dcl.meaning.general]/3.5: a block-scope
// function declaration or `extern` variable declares a member of the
// innermost enclosing namespace, so in a function of an unnamed namespace
// (a member function, a lambda and a local class's member function there
// included) it has internal linkage, and its later namespace-scope
// definition is an internal symbol; in a `static` function outside one it
// has external linkage (the function's own linkage does not matter); one
// that redeclares an earlier `static` variable takes its internal linkage.
// Compared with clang's IR.
//@ expect: ir
namespace {
void g() {
  extern int v;
  extern void h();
  v = 1;
  h();
}
int v = 5;
void h() {}

struct A {
  void m() {
    extern int mv;
    mv = 2;
  }
};
int mv;

void lam() {
  [] {
    extern int lv;
    lv = 3;
  }();
}
int lv;

void loc() {
  struct L {
    static void f() {
      extern void lf();
      lf();
    }
  };
  L::f();
}
void lf() {}
} // namespace

static void s() {
  extern int sv;
  extern void sh();
  sv = 4;
  sh();
}
int sv = 6;
void sh() {}

static int pv = 7;
void p() {
  extern int pv;
  pv = 8;
}

void use() {
  g();
  A().m();
  lam();
  loc();
  s();
  p();
}
