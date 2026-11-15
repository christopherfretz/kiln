// [over.match.ctor]/1, [over.best.ics]/4: direct-initializing `B` from an
// `int` with constructors `B(...)` and `B(const B&)` calls `B(...)`: the
// copy constructor's parameter is not initialized by a user-defined
// conversion through `B(...)` here, so no temporary is copied. Found
// running GCC's C++ testsuite. Matches clang 19 and GCC.
//@ expect: run
//@ strict-diags
//@ xfail(kiln): WP-1484, overload resolution takes B(const B&) through a B(...) user conversion of the argument
//@ stdout: 2 0
extern "C" int printf(const char*, ...);
int made, copies;
struct B {
  B(...) { ++made; }
  B(const B&) { ++copies; }
  ~B() {}
};
int main() {
  B b(1);
  B c(1, 2);
  printf("%d %d\n", made, copies);
}
