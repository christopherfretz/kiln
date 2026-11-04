//@ args: -std=c++26
// #pragma GCC visibility push(v) / pop (spec/codegen.md "Symbol
// visibility"; clang Sema::AddPushedVisibilityAttribute): a declaration
// in the scope of a push without its own visibility gets the pushed one,
// which is then that entity's explicit visibility for every later
// redeclaration and definition. A declaration first seen with an
// explicit attribute keeps it. A namespace's attribute is popped at its
// closing brace, before a pragma that follows it.
__attribute__((visibility("default"))) void f();
#pragma GCC visibility push(hidden)
void f();
void g();
inline int inl() {
  static int s = 1;
  return s++;
}
int v;
extern int ev;
struct S {
  void m();
  static int sm;
  virtual ~S();
};
template <class T> struct TT {
  void m() {}
};
template <class T> void tf() {}
namespace __attribute__((visibility("default"))) N {
void nf();
}
#pragma GCC visibility push(protected)
void prot() {}
#pragma GCC visibility pop
#pragma GCC visibility pop
void f() {}
void g() {}
void S::m() {}
int S::sm;
S::~S() {}
void after() {}
int use() {
  tf<int>();
  TT<int>().m();
  N::nf();
  return inl() + ev;
}
