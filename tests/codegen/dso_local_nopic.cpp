//@ args: -fno-pic
// dso_local under -fno-pic (spec/codegen.md "Symbol visibility and
// `dso_local`"; clang shouldAssumeDSOLocal): with the static relocation
// model nothing can be preempted, so every definition and every
// declaration is dso_local, except declarations of thread_local
// variables; there is no "PIC Level" module flag.
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
