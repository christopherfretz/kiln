//@ args: -fPIE
// dso_local under -fPIE, the driver's default on Linux (spec/codegen.md
// "Symbol visibility and `dso_local`"; clang shouldAssumeDSOLocal): definitions are dso_local (an executable
// cannot be interposed); declarations of functions are not (they may come
// from a shared library through the PLT); declarations of non-TLS
// variables are dso_local only where direct access to external data is
// assumed. The module carries "PIC Level" and "PIE Level" 2.
extern int ev;
extern thread_local int etl;
int gv = 1;
thread_local int tl = 2;
extern "C" void ext();
void f() {}
inline int inl() { return 3; }
static int st() { return 4; }
void (*g)() = f;
struct S {
  virtual ~S();
  virtual void v() {}
};
S::~S() {}
int use() {
  ext();
  static int sl = inl();
  try {
    throw 1;
  } catch (...) {
  }
  return ev + etl + st() + sl + tl;
}
