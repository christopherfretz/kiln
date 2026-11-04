//@ args: -fPIC -fsemantic-interposition -fdirect-access-external-data
// -fsemantic-interposition and -fdirect-access-external-data under
// -fPIC (spec/codegen.md "Symbol visibility and `dso_local`"): the first
// adds the "SemanticInterposition" module flag; the second only adds the
// "direct-access-external-data" module flag (it makes variable
// declarations dso_local only under -fPIE or -fno-pic).
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
