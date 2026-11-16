// [over.match.ctor]/1, [over.ics.rank]/2.1, [class.copy.elision]: direct-
// initializing `B` from an `int` with constructors `B(...)` and
// `B(const B&)` selects the copy constructor (a user-defined conversion
// through `B(...)` ranks above the ellipsis conversion of calling `B(...)`
// directly), and the copy of that temporary is elided: the temporary is
// constructed in place of `b` (clang's and GCC's CWG2327 practice; C++14
// [class.copy]/31.3). So `B(...)` runs once per object and no copy is made.
// Found running GCC's C++ testsuite. Matches clang 19 and GCC.
//@ expect: run
//@ strict-diags
//@ stdout: 2 0 2
extern "C" int printf(const char*, ...);
int made, copies, dtors;
struct B {
  B(...) { ++made; }
  B(const B&) { ++copies; }
  ~B() { ++dtors; }
};
int main() {
  {
    B b(1);
    B c(1, 2);
  }
  printf("%d %d %d\n", made, copies, dtors);
}
