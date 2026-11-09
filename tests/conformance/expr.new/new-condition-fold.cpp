// [expr.new]/14 (a replaceable allocation may be omitted), [stmt.if],
// [expr.cond], [expr.log.and], [expr.log.or], [expr.unary.op]: a
// new-expression whose value only feeds a condition is folded to `true`
// outside a constant context, as clang's codegen does
// (`ConstantFoldsToSimpleInteger`; WP-1108, spec/codegen.md §3): the
// allocation is omitted when the condition folds, and kept where it does
// not (a non-constant operand of `&&`/`||`, a non-constexpr constructor,
// a non-constant initializer). The program still computes the same.
//@ expect: run
//@ stdout: 127 4 1
#include <cstdio>
#include <cstdlib>
#include <new>
static int allocs;
void *operator new(std::size_t n) { ++allocs; return std::malloc(n ? n : 1); }
void *operator new(std::size_t n, const std::nothrow_t &) noexcept { ++allocs; return std::malloc(n ? n : 1); }
void operator delete(void *p) noexcept { std::free(p); }
void operator delete(void *p, std::size_t) noexcept { std::free(p); }
struct K { constexpr K() : x(1) {} int x; };
struct N { N() : x(2) {} int x; };
int calls;
int g() { return ++calls; }
int main(int argc, char **) {
  bool c = argc > 5;
  int r = 0;
  if (new int(4)) r += 1;
  r += new (std::nothrow) int(1) ? 2 : 100;
  r += (c || new int(5)) ? 4 : 100;
  r += (!c && new K) ? 8 : 100;
  r += !new int ? 100 : 16;
  r += new N ? 32 : 100;
  r += new int(g()) ? 64 : 100;
  std::printf("%d %d %d\n", r, allocs, calls);
}
