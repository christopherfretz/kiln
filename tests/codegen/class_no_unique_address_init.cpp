// Initializing a [[no_unique_address]] member: a call's direct return is
// stored with the member's data size (clang CreateCoercedStore), copies
// use the data size too.
struct Empty {};
struct Tail { int a; char b; Tail(int x) : a(x), b(2) {} };
Tail mk(int);
struct S {
  [[no_unique_address]] Tail t;
  char c;
  S() : t(mk(1)), c(3) {}
  S(int x) : t(x), c(4) {}
};
struct Copy {
  [[no_unique_address]] Tail t;
  char c;
  int z;
};
template <class T> struct Holder { [[no_unique_address]] T v; int n; };
struct Agg { [[no_unique_address]] Tail t; char c; };
int f() {
  S a, b(2);
  Copy c{Tail(1), 2, 3};
  Copy d = c;
  d = c;
  Holder<Empty> h{{}, 4};
  Holder<Empty> h2 = h;
  Agg g{mk(4), 5};
  return a.c + b.c + d.z + h2.n + g.c + sizeof(Copy);
}
