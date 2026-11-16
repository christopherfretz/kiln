// [class.copy.elision]/1, clang's `-fno-elide-constructors` from C++17: the
// optional elisions are not performed, so a named return value is moved
// into the return slot (one move, two destructions) and the copy of a
// temporary made by a user-defined conversion for a copy constructor
// (CWG2327) is made; guaranteed elision of prvalues ([dcl.init.general]/
// 16.6.1, [stmt.return]) is unaffected. Before C++17 the flag is rejected
// (kiln builds no copies of temporaries there; spec/driver.md). Found
// running GCC's C++ testsuite. Matches clang 19 and 22 and GCC 16.1.
//@ expect: run
//@ std: c++17
//@ args: -fno-elide-constructors
//@ strict-diags
//@ stdout-file: no-elide-constructors-run.stdout
extern "C" int printf(const char*, ...);
int made, copies, moves, dtors;
struct S {
  S() { ++made; }
  S(const S&) { ++copies; }
  S(S&&) { ++moves; }
  ~S() { ++dtors; }
};
struct B {
  B(...) { ++made; }
  B(const B&) { ++copies; }
  ~B() { ++dtors; }
};
S nrvo() { S s; return s; }
S nrvo2(bool b) { S s; if (b) return s; return s; }
S prvalue() { return S(); }
void take(S) {}
void p(const char* w) {
  printf("%s %d %d %d %d\n", w, made, copies, moves, dtors);
  made = copies = moves = dtors = 0;
}
int main() {
  { S s = nrvo(); } p("nrvo");
  { S s = nrvo2(true); } p("nrvo-two-returns");
  { S s = prvalue(); } p("prvalue-return");
  { S s = S(S()); } p("nested-prvalue");
  { take(S()); } p("prvalue-argument");
  { B b(1); } p("udc-copy");
  { S* q = new S(nrvo()); delete q; } p("new-from-call");
}
