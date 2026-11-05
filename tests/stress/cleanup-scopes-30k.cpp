// [stmt.block], [stmt.jump]/2, [except.ctor]/2, spec/codegen.md: one
// function of 30,000 blocks, each destroying a local after a call that may
// throw, so every block pushes and pops a cleanup with both a normal and an
// EH path. The shape is a `template for` over many members expanded into one
// body, as a binding generator does. Popping a cleanup merges its entry
// block into a sole predecessor (clang `SimplifyCleanupEntry`); finding
// that predecessor and redirecting the branches to it once scanned the
// whole function, so lowering was quadratic: 170 s here, past the runner's
// compile timeout. Linear, it lowers in under 3 s (the whole compile,
// mostly LLVM's -O0 backend, takes about 10 s, as with clang).
//@ expect: run
//@ skip(clang): kiln-only scale check
//@ oracle: none
//@ mem<=768
int live, made, thrown;
struct D {
  D() { ++live, ++made; }
  ~D() { --live; }
};
[[gnu::noinline]] void g(int i) {
  if (i == thrown) throw i;
}
#define U { D d; g(1); }
#define A U U U U U U U U U U
#define B A A A A A A A A A A
#define C B B B B B B B B B B
#define E C C C C C C C C C C
void f() { E E E }
int main() {
  f();
  if (live != 0 || made != 30000) return 1;
  thrown = 1;
  try {
    f();
  } catch (int) {
  }
  return live == 0 && made == 30001 ? 0 : 2;
}
