// C++14 [expr.const]/2: a new-expression or delete-expression is not a core
// constant expression before C++20, but only an evaluated one disqualifies
// an expression: a constexpr function that allocates on a branch a constant
// evaluation does not take is still constant there ([dcl.constexpr]/5 asks
// for one constant argument set), and outside a constant expression
// allocation is ordinary dynamic initialization. clang 22 and GCC 16.1
// accept. Found running GCC's C++ testsuite.
//@ args: -std=c++14
//@ strict-diags

constexpr int sometimes(bool b) {
  if (b) {
    int *p = new int(2);
    int r = *p;
    delete p;
    return r;
  }
  return 1;
}
static_assert(sometimes(false) == 1, "");
constexpr int u = sometimes(false);

constexpr int pick(bool b) { return b ? *new int(3) : 4; }
static_assert(pick(false) == 4, "");

int *heap = new int(5);
const int d = (delete new int(6), 1);
int run = sometimes(true) + pick(true);

struct Member { int *p = new int(7); int q = 8; };
constexpr int unused(int i) { return i; }
static_assert(unused(9) == 9, "");
