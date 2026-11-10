// [class.temporary]/7 (C++23, P2718R0), [intro.execution]/5: the
// temporaries of a default argument used in the for-range-initializer
// persist until the end of the loop, whatever expression holds them. The
// use gets a copy of the default argument with fresh temporaries
// (spec/sema.md §7.6a), so every node on the way down to one is copied:
// here a compound assignment, a rewritten `!=`, a new-expression, a
// delete-expression, `__builtin_bit_cast`, `__builtin_choose_expr`, GNU
// `?:` (its opaque value rebound to the copied common expression) and an
// instantiated fold. Before WP-1166 these kept the shared node and its
// temporary died with the initializer (`~D1 1 2`). clang 19 and 22 agree.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ args: -Wno-gnu-conditional-omitted-operand
//@ stdout: 1 2 ~D1 | 1 2 ~D3 ~D2 | 1 2 ~D4 | 1 2 ~D5 | 1 2 ~D6 | 1 2 ~D7 | 1 2 ~D8 | 1 2 ~D10 ~D9 | end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  int* p = nullptr;
  ~D() { printf("~D%d ", v); }
  bool operator==(const D&) const { return false; }
};
struct A {
  int a[2]{1, 2};
  const int* begin() const { return a; }
  const int* end() const { return a + 2; }
};
int g;
A ca(int = (g += D{1}.v)) { return {}; }
A rw(bool = D{2} != D{3}) { return {}; }
A nw(int* = new int(D{4}.v)) { return {}; }
A dl(int = (delete D{5}.p, 0)) { return {}; }
A bc(int = __builtin_bit_cast(int, D{6}.v)) { return {}; }
A ch(int = __builtin_choose_expr(1, D{7}.v, 0)) { return {}; }
A bco(int = D{8}.v ?: 0) { return {}; }
template <int... N> A fo(int = (D{N}.v + ...)) { return {}; }

int main() {
  for (int x : ca())
    printf("%d ", x);
  printf("| ");
  for (int x : rw())
    printf("%d ", x);
  printf("| ");
  for (int x : nw())
    printf("%d ", x);
  printf("| ");
  for (int x : dl())
    printf("%d ", x);
  printf("| ");
  for (int x : bc())
    printf("%d ", x);
  printf("| ");
  for (int x : ch())
    printf("%d ", x);
  printf("| ");
  for (int x : bco())
    printf("%d ", x);
  printf("| ");
  for (int x : fo<9, 10>())
    printf("%d ", x);
  printf("| end\n");
}
