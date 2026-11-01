// [intro.execution]/5, [class.temporary]/6.9: a temporary created in a
// default argument persists until the end of the full-expression
// containing the call (each element of an array is its own; clang).
//@ expect: run
//@ stdout: c1 g1 ~1 | c2 T2 ~2 | c3 T3 ~3 c4 T4 ~4 | end
extern "C" int printf(const char *, ...);
int next = 1;
struct X {
  int id;
  X() : id(next++) { printf("c%d ", id); }
  ~X() { printf("~%d ", id); }
};
void g(const X &x = X()) { printf("g%d ", x.id); }
struct T { T(const X &x = X()) { printf("T%d ", x.id); } };
int main() {
  g();
  printf("| ");
  T t;
  printf("| ");
  T a[2];
  printf("| end\n");
}
