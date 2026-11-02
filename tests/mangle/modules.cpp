// [module.unit]/7, Itanium ABI <module-name> (clang `mangleModuleName`):
// a namespace-scope entity with linkage attached to named module `A.B`
// carries `W1AW1B` before its unqualified name, each module-name prefix is
// a substitution candidate, and entities in `extern "C++"` or with
// internal linkage carry none. A non-exported `const` variable attached to
// the module has module linkage (P2788), not internal linkage.
export module A.B;
export int f(int x) { return x; }
int g() { return 1; }
static int h() { return 2; }
export namespace ns {
int k() { return 3; }
struct S {
  int m();
  static int sv;
  S();
  ~S();
};
int S::m() { return 4; }
int S::sv = 5;
S::S() {}
S::~S() {}
} // namespace ns
export template <class T> T tw(T t) { return t; }
export int var = 7;
export const int ci = 9;
export inline int ii() { return 10; }
export struct X {
  int v;
};
export int takes(X, X *, ns::S &) { return 0; }
namespace ns2 {
export int q();
}
int ns2::q() { return 1; }
export extern "C++" int ecpp() { return 6; }
export extern "C" int ec() { return 11; }
const int mc = 12;
export const int *use_mc() { return &mc; }
export int use() { return tw(1) + tw(2L) + h() + g() + ii() + var + ci; }
