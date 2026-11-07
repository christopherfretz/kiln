// [expr.cond]/6, [over.built]/24-25: arms of different types, one of class
// type, that cannot be converted to match each other ([expr.cond]/4) are
// converted by overload resolution among the built-in candidates
// `T operator?:(bool, T, T)` (T a pointer, pointer-to-member or scoped
// enumeration type) and `LR operator?:(bool, L, R)` (promoted arithmetic
// L, R). Reduced from clang's PPDirectives.cpp:
// `lexer ? *lexer->getFileEntry() : static_cast<FileEntry *>(nullptr)`,
// where FileEntryRef has `operator const FileEntry *()`.
//@ expect: run
//@ stdout: 1 1 7 2.5 2 1
#include <cstdio>

struct F { int v; };
struct Ref {
  F *p;
  operator const F *() const { return p; }
};

enum class E { a, b, c };
struct ToE {
  operator E() const { return E::c; }
};

struct ToInt {
  int v;
  operator int() const { return v; }
};

struct M { int x, y; };
struct ToMem {
  operator int M::*() const { return &M::y; }
};

const F *pick(bool b, Ref r) { return b ? r : static_cast<F *>(nullptr); }

int main() {
  F f{7};
  Ref r{&f};
  const F *x = pick(true, r);
  const F *y = pick(false, r);
  // The class arm converts by its conversion function, then the usual
  // arithmetic conversions give `double`.
  double d = false ? ToInt{7} : 2.5;
  E e = true ? ToE{} : E::a;
  M m{1, 2};
  int M::*pm = true ? ToMem{} : &M::x;
  std::printf("%d %d %d %g %d %d\n", x == &f, y == nullptr, x->v, d, static_cast<int>(e),
              m.*pm == 2 ? 1 : 0);
}
