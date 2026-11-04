//@ args: -fPIC -fvisibility=hidden
// dso_local under -fPIC -fvisibility=hidden (spec/codegen.md "Symbol
// visibility and `dso_local`"): hidden definitions are dso_local even in a
// shared library; declarations defined elsewhere stay default and so are
// not.
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
