// [intro.execution]/5, [class.temporary]/4: outside a lifetime-extending
// context the temporaries of a default member initializer that an
// aggregate initialization uses are destroyed at the end of the
// full-expression, as clang 22 does: they are not rebuilt for the use
// (spec/sema.md §7.6a). A non-aggregate's member initializer runs in its
// constructor. WP-1177.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ requires: clang>=22
//@ stdout: ~D4 use | ~D4 ~D5 useo | ~D4 decl | ~D4 refdecl | ~D6 k6 | ~D4 1 2 | end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
struct A {
  int a[2]{1, 2};
  const int* begin() const { return a; }
  const int* end() const { return a + 2; }
};
struct Agg {
  int x = D{4}.v;
};
struct Outer {
  Agg in;
  int y = D{5}.v;
};
struct NonAgg {
  int x = D{6}.v;
  NonAgg() {}
};
void use(const Agg&) { printf("use "); }
void useo(const Outer&) { printf("useo "); }
int kn(const NonAgg& n) { return n.x; }
struct Arr {
  Agg a[2];
};
A mk(const Arr&) { return {}; }

int main() {
  use(Agg{});
  printf("| ");
  useo(Outer{});
  printf("| ");
  {
    Agg a{};
    printf("decl | ");
  }
  {
    const Agg& a = Agg{};
    printf("refdecl | ");
  }
  int k = kn(NonAgg{});
  printf("k%d | ", k);
  // An array filler is one initializer for every element: clang 22 does
  // not rebuild it, and neither does kiln.
  for (int x : mk(Arr{}))
    printf("%d ", x);
  printf("| end\n");
}
