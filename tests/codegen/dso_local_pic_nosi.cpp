//@ args: -fPIC -fno-semantic-interposition
// -fno-semantic-interposition (spec/codegen.md "Symbol visibility and
// `dso_local`"; clang shouldAssumeDSOLocal, canBenefitFromLocalAlias):
// under -fPIC an external definition of a function that is not in a
// COMDAT may be called through a local alias, so it is dso_local;
// variables, inline functions and declarations are not.
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
