// [class.temporary]/8 (CWG3043, C++26 DIS), [stmt.expand]/5.1: a
// temporary created in an element E of an enumerating expansion-init-list
// persists for the lifetime of the for-range-declaration E initializes,
// whatever its type, and is destroyed after it, in reverse order of
// construction ([class.temporary]/11). Each element `{ decl = E_i; body }`
// destroys its own temporaries at the end of its block, after the body, so
// `D{8}.self()` and a view of an owner temporary do not dangle (and
// -Wdangling-gsl does not warn). GCC 16.1 implements the earlier
// resolution (no extension: `~D2 x2`); clang 22 has no P1306. The golden is
// the DIS order. WP-1106; spec/sema.md §7.6a.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: none
//@ stdout: x2 ~D2 | x1 ~D1 x2 ~D2 x3 ~D3 | r4 ~D4 r5 ~D5 | f6 ~D6 f7 ~D7 | s8 ~D8 | p1 ~D10 ~D9 | t3 t5 ~D5 ~D4 | v1 ~O1 v2 ~O2 | b12 ~T | u0 ~D0 u1 ~D1 ~D0 | n7 ~D7 n8 ~D8 | end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
  const D& self() const { return *this; }
};
D mk(int v) { return D{v}; }
struct P { int a, b; };
struct [[gsl::Owner(int)]] Own {
  int v;
  ~Own() { printf("~O%d ", v); }
};
struct [[gsl::Pointer(int)]] View {
  const int* p;
  View(const Own& o) : p(&o.v) {}
};
struct T {
  P p;
  ~T() { printf("~T "); }
};

// Each element of a type-dependent list, instantiated: the element's
// temporaries are its own, not the pattern's.
template <class T> void dep(T t) {
  template for (auto x : {T{t.v}.v, T{t.v + 1}.v}) printf("u%d ", x);
}
template <int N> void val() {
  template for (const auto& x : {D{N}.self(), D{N + 1}.self()}) printf("n%d ", x.v);
}

int main() {
  // By value: the temporary outlives the copy.
  template for (auto x : {D{2}.v}) printf("x%d ", x);
  printf("| ");
  // Multiple elements: each element's temporary ends with its block.
  template for (auto x : {D{1}.v, D{2}.v, D{3}.v}) { printf("x%d ", x); }
  printf("| ");
  // const auto&: bound directly ([class.temporary]/6 too).
  template for (const auto& x : {D{4}, D{5}}) { printf("r%d ", x.v); }
  printf("| ");
  // auto&&: a prvalue and a returned prvalue.
  template for (auto&& x : {D{6}, mk(7)}) { printf("f%d ", x.v); }
  printf("| ");
  // A reference to the temporary through a member call does not dangle.
  template for (const auto& x : {D{8}.self()}) { printf("s%d ", x.v); }
  printf("| ");
  // Two temporaries in one element (`&&` sequences them): reverse order
  // of construction.
  template for (int x : {D{9}.v && D{10}.v}) printf("p%d ", x);
  printf("| ");
  // A mixed list: an element with no temporary, then two.
  template for (auto x : {3, (D{4}.v && D{5}.v) + 4}) printf("t%d ", x);
  printf("| ");
  // A view of an owner temporary does not dangle (no -Wdangling-gsl).
  template for (View x : {Own{1}, Own{2}}) printf("v%d ", *x.p);
  printf("| ");
  // A structured-binding declaration: the object holds a copy of `p`.
  template for (auto [a, b] : {T{{1, 2}}.p}) printf("b%d%d ", a, b);
  printf("| ");
  dep(D{0});
  printf("| ");
  val<7>();
  printf("| end\n");
}
