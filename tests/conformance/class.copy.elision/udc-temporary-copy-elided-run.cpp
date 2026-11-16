// [class.copy.elision], CWG2327: when a copy or move constructor of `T` is
// selected for a complete object and its argument is a temporary `T` made by
// a user-defined conversion (a constructor of `T`, a constructor template, an
// initializer-list constructor, a conversion function), the copy is elided
// and the temporary built in place, in every complete-object context
// (variable, `new`, functional cast, member, return). A base-class
// initializer is not a complete object: clang copies there (GCC 16.1 elides
// that copy too). Found running GCC's C++ testsuite. Matches clang 19.
//@ expect: run
//@ strict-diags
//@ stdout-file: udc-temporary-copy-elided-run.stdout
#include <initializer_list>
extern "C" int printf(const char*, ...);
int made, copies, moves, dtors;
struct B {
  B(...) { ++made; }
  B(const B&) { ++copies; }
  ~B() { ++dtors; }
};
struct T {
  template <class U> T(U&&, ...) { ++made; }
  T(const T&) { ++copies; }
  T(T&&) { ++moves; }
  ~T() { ++dtors; }
};
struct L {
  L(std::initializer_list<int>) { ++made; }
  L(const L&) { ++copies; }
  ~L() { ++dtors; }
};
struct X { operator B() const { return B(0); } };
struct D : B { D() : B(1) {} };
struct M { B b; M() : b(1) {} };
struct N { [[no_unique_address]] B b; int i; N() : b(1), i(0) {} };
B ret() { return 1; }
void p(const char* s) {
  printf("%s %d %d %d %d\n", s, made, copies, moves, dtors);
  made = copies = moves = dtors = 0;
}
int main() {
  { B b(1); } p("direct");
  { B b = 1; } p("copy-init");
  { T t(1); } p("template");
  { L l({1, 2}); } p("init-list");
  { B b(X{}); } p("conversion-function");
  { ret(); } p("return");
  { M m; } p("member");
  { N n; } p("no-unique-address");
  { B* q = new B(1); delete q; } p("new");
  { B(1); } p("functional-cast");
  { D d; } p("base");
}
