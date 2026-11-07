// [expr.new]/19: the allocation call is an ordinary function call, so an
// allocation function's parameters past the placement arguments take
// their default arguments ([dcl.fct.default]; clang `BuildCXXNew` ->
// `GatherArgumentsForCall`): a class's own (LLVM's YAML `Node::operator
// new(size_t, BumpPtrAllocator &, size_t Alignment = 16)`), a class
// template's (instantiated with it), and a global one, also from a
// function template's instantiation.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: k=8 k=1 g7 5 4 16 1 1 2
#include <cstddef>
#include <cstdio>
#include <cstdlib>
struct Alloc { size_t last = 0; void *Allocate(size_t s, size_t al) { last = al; return std::malloc(s); } };
struct Node {
  void *operator new(size_t Size, Alloc &A, size_t Alignment = 16) noexcept { return A.Allocate(Size, Alignment); }
  void operator delete(void *Ptr, Alloc &A, size_t Size) noexcept {}
  void operator delete(void *) = delete;
  int x = 3;
};
struct Null : Node { Null(int v) { x = v; } };
template <class T> struct TA { void *operator new(size_t s, T t, int k = sizeof(T)) { std::printf("k=%d ", k); return std::malloc(s); } int v = 1; };
void *operator new(size_t s, Alloc &a, int lvl, const char *tag = "g") { std::printf("%s%d ", tag, lvl); return std::malloc(s); }
template <class T> T *mk(Alloc &a) { return new (a) T(4); }
template <class T> TA<T> *mk2(T t) { return new (t) TA<T>; }
int main() {
  Alloc a;
  Node *n = new (a) Null(5);
  Node *m = mk<Null>(a);
  TA<long> *t = mk2<long>(1);
  TA<char> *u = new ('c') TA<char>;
  int *i = new (a, 7) int(2);
  std::printf("%d %d %zu %d %d %d\n", n->x, m->x, a.last, t->v, u->v, *i);
}
