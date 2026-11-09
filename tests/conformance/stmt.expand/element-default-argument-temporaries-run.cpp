// [class.temporary]/8 (CWG3043, C++26 DIS), [intro.execution]/5,
// [stmt.expand]/5.1: a default argument's temporaries are created in the
// full-expression of the call that uses it, so a default argument used in
// an element of an enumerating expansion-init-list (or in a destructuring
// expansion-initializer) creates them in a lifetime-extending context: they
// persist with the for-range-declaration, as the element's other
// temporaries do. Each use gets its own; a nested default argument's too;
// a use outside an expansion statement ends with its call. At run time, in
// a template and in constant evaluation (a reference to a default
// argument's temporary stays usable in the body). GCC 16.1 implements the
// earlier resolution (no extension of element temporaries: `~D7 w7 w8`,
// and it rejects `h()` as reading an object outside its lifetime); clang 22
// has no P1306. The golden is the DIS order. WP-1149; spec/sema.md §7.6a.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: none
//@ stdout: a1 a2 ~D1 | w7 ~D7 w8 | n8 ~D7 n9 | m18 ~D9 ~D9 m1 | ~D7 k7 | t1 t2 ~D1 | u7 ~D7 u0 | t1 t2 ~D1 | u7 ~D7 u0 | end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
struct P { int a, b; };
P mk(const D& = D{1}) { return {1, 2}; }
int dv(const D& d = D{7}) { return d.v; }
int outer(int x = dv()) { return x + 1; }
int dw(const D& d = D{9}) { return d.v; }

template <class U> void tpl() {
  template for (auto x : mk()) printf("t%d ", x);
  printf("| ");
  template for (U x : {dv(), 0}) printf("u%d ", int(x));
  printf("| ");
}

constexpr const int& pass(const int& x = int{5}) { return x; }
constexpr int h() {
  int s = 0;
  template for (const int& x : {pass(), 1}) s += x;
  return s;
}
static_assert(h() == 6);
template <class U> constexpr int ht() {
  U s = 0;
  template for (const int& x : {pass(), 1}) s += x;
  return int(s);
}
static_assert(ht<int>() == 6 && ht<long>() == 6);

int main() {
  // Destructuring: created in the expansion-initializer.
  template for (auto x : mk()) printf("a%d ", x);
  printf("| ");
  // Enumerating: created in the element's initializer.
  template for (auto x : {dv(), 8}) printf("w%d ", x);
  printf("| ");
  // A default argument's own default argument.
  template for (auto x : {outer(), 9}) printf("n%d ", x);
  printf("| ");
  // Two uses of one default argument.
  template for (auto x : {dw() + dw(), 1}) printf("m%d ", x);
  printf("| ");
  int k = dv();
  printf("k%d | ", k);
  tpl<int>();
  tpl<long>();
  printf("end\n");
}
