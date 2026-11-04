// dso_local (spec/codegen.md "Symbol visibility", "dso_local"; clang
// shouldAssumeDSOLocal): without -fPIC/-fPIE (the oracle's default is
// static relocation model) every definition and every non-TLS
// declaration is dso_local, except declarations of thread_local
// variables; local-linkage symbols are always dso_local.
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
