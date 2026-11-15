// GNU statement expression of class type: its result is a temporary of
// the enclosing full-expression, destroyed after the temporaries of the
// result's own full-expression (clang's `BuildStmtExpr` binds it), and a
// throwing destructor of one of those temporaries destroys the result
// first. Found running GCC's C++ testsuite.
//@ xfail(clang): clang 19 and 22 leak the result (GCC 16 too)
//@ expect: run
//@ oracle: none
//@ stdout: 1 2 3 ~2 ~1 ~3 | 1 2 3 ~2 ~3 ~1 caught 0
extern "C" int printf(const char*, ...);
int live, boom;
struct A {
  int i;
  A(int i) : i(i) { ++live; printf("%d ", i); }
  A(const A&) = delete;
  ~A() noexcept(false) {
    --live;
    printf("~%d ", i);
    if (i == boom) throw 0;
  }
  A operator+(const A&) const { return A(3); }
};
int main() {
  ({ A(1) + A(2); });
  printf("| ");
  boom = 2;
  try { ({ A(1) + A(2); }); } catch (int) { printf("caught "); }
  printf("%d\n", live);
}
