//@ args: -O1 -Xclang -disable-llvm-passes
// [dcl.init.aggr]/5, [dcl.init.general]/9, [basic.lval]/11: the stores
// that value-initialize the elements and members an aggregate
// initializer leaves out (the array filler loop included), and the
// value-initialization of `new T()`, carry the access's `!tbaa` tag at
// -O1 (clang `EmitStoreThroughLValue`).
int g(int);
struct Q { int a; float b; };
struct S { int a; int b; int* p; };
struct T { int x{}; long y = long(); };
void h(int*, long*, Q*, int*);
void k(S*, T*, int*);
void f(int x) {
  int arr[4] = {g(1)};
  long l[2] = {x};
  Q q[2] = {{x, 1.0f}};
  int big[8] = {g(2), g(3), g(4), g(5), g(6), g(7)};
  h(arr, l, q, big);
}
void m(int x) {
  S s = {x};
  T t;
  int* n = new int();
  k(&s, &t, n);
}
