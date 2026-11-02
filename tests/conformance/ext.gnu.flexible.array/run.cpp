// Flexible array members (spec/sema.md §5.8): the member has size zero at
// the aligned end of the preceding members and counts for the class's
// alignment; it is accessed past the object's size; an implicit copy
// copies only the other members.
//@ expect: run
//@ stdout: 4 4 8 8 4 0 4 8 8 | 10 4 0 | 2
extern "C" int printf(const char*, ...);
extern "C" void* malloc(decltype(sizeof 0));
extern "C" void free(void*);

struct A { int n; char d[]; };
struct L { char c; long d[]; };
union U { short s; int d[]; };
struct E { char d[]; };
struct W { int x; A a; };
template <class T> struct X { int n; T d[]; };

static_assert(sizeof(A) == 4 && __builtin_offsetof(A, d) == 4);
static_assert(sizeof(L) == 8 && alignof(L) == 8);

int sum(const A* a) {
  int s = 0;
  for (int i = 0; i < a->n; ++i) s += a->d[i];
  return s;
}

int main() {
  A* a = static_cast<A*>(malloc(sizeof(A) + 4));
  a->n = 4;
  for (int i = 0; i < 4; ++i) a->d[i] = char(i + 1);
  A b = *a;
  printf("%zu %zu %zu %zu %zu %zu %zu %zu %zu | ", sizeof(A), __builtin_offsetof(A, d),
         sizeof(L), __builtin_offsetof(L, d), sizeof(U), sizeof(E), alignof(U), sizeof(W),
         alignof(L));
  static A g = {7, {}};
  printf("%d %d %d | ", sum(a), b.n, g.n - 7);
  X<short>* x = static_cast<X<short>*>(malloc(sizeof(X<short>) + 2 * sizeof(short)));
  x->d[1] = 2;
  printf("%d\n", x->d[1]);
  free(x);
  free(a);
}
