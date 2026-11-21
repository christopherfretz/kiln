// new- and delete-expressions spec/codegen.md §3.1 once listed as
// unsupported ([expr.new], [expr.delete]): a pointer-to-member element
// type, a string literal initializing a char array, a placement argument
// of class type inside a conditional, a non-constant align_val_t
// argument, and a class whose only sized operator delete is not a usual
// deallocation function. Emitted as clang does (WP-1782).
using size_t = decltype(sizeof 0);
namespace std { enum class align_val_t : size_t {}; }
void *operator new(size_t, std::align_val_t);
struct S { int a; };
struct Arena { int n; };
void *operator new(size_t, Arena);
int S::**mp() { return new (int S::*)(&S::a); }
int S::**mpa() { return new (int S::*[3]); }
char *sl() { return new char[4]{"abc"}; }
char *sl2() { return new char[]{"abcd"}; }
int *pc(bool c, Arena a, Arena b) { return c ? new (a) int(1) : new (c ? a : b) int(2); }
void *al(size_t n) { return ::operator new(n, std::align_val_t(n)); }
struct D {
  int x;
  void operator delete(void *, size_t, int);
  void operator delete(void *);
};
void del(D *d) { delete d; }
