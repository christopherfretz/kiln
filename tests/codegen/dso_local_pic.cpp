//@ args: -fPIC
// dso_local under -fPIC (spec/codegen.md "Symbol visibility and
// `dso_local`"; clang shouldAssumeDSOLocal): a shared-library build may have any
// default-visibility symbol interposed, so only local-linkage and hidden
// or protected symbols are dso_local; a default-visibility function's
// address (g's initializer) is not. The module carries "PIC Level" 2.
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
