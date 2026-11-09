// [class.temporary]/9 (CWG3043), [stmt.expand]/5.3: a temporary created in
// a destructuring expansion-initializer persists for the lifetime of the
// reference that holds the expansion-initializer's value, that is until
// the end of the whole expansion statement, after its last element, at
// run time and in constant evaluation. The init-statement's temporaries end
// with its own full-expression, before the first element. An iterating
// statement's range is a `static constexpr` variable whose temporaries are
// part of its constant initialization. GCC 16.1 agrees; clang 22 has no
// P1306. WP-1106; spec/sema.md §7.6a.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ stdout: d4 d5 ~W1 | q6 q7 ~Q ~W2 | ~W3 i1 | a1 a2 | 45w|67qw|wi|end
extern "C" int printf(const char*, ...);
struct W {
  int v;
  ~W() { printf("~W%d ", v); }
};
struct P { int a, b; };
P mkp(const W&) { return {4, 5}; }
struct Q {
  int a, b;
  ~Q() { printf("~Q "); }
};
Q mkq(const W&) { return {6, 7}; }
int zero(const W&) { return 0; }
struct Arr {
  int a[2]{1, 2};
  constexpr const int* begin() const { return a; }
  constexpr const int* end() const { return a + 2; }
};
struct CW {
  int v;
  constexpr ~CW() {}
};
constexpr Arr mka(const CW&) { return {}; }

struct L {
  char buf[32]{};
  int n = 0;
  constexpr void put(char c) { buf[n++] = c; }
};
struct LW {
  L* l;
  char c;
  constexpr ~LW() { l->put(c); }
};
struct LQ {
  L* l;
  int a, b;
  constexpr ~LQ() { l->put('q'); }
};
constexpr P lp(const LW&) { return {4, 5}; }
constexpr LQ lq(const LW& w) { return {w.l, 6, 7}; }
constexpr int lz(const LW&) { return 0; }
constexpr void digit(L& l, int x) { l.put(char('0' + x)); }
constexpr void digit(L&, L*) {}
constexpr L run() {
  L l;
  template for (auto x : lp(LW{&l, 'w'})) l.put(char('0' + x));
  l.put('|');
  template for (auto x : lq(LW{&l, 'w'})) digit(l, x);
  l.put('|');
  template for (int i = lz(LW{&l, 'w'}); auto x : {i + 1}) l.put(char('h' + x));
  l.put('|');
  return l;
}
constexpr L k = run();
static_assert(k.buf[0] == '4' && k.buf[2] == 'w');

int main() {
  // The temporary W outlives every element.
  template for (auto x : mkp(W{1})) { printf("d%d ", x); }
  printf("| ");
  // A class prvalue's own temporary: destroyed before the extended W.
  template for (auto x : mkq(W{2})) { printf("q%d ", x); }
  printf("| ");
  // The init-statement is a full-expression of its own.
  template for (int x = zero(W{3}); auto y : {x + 1}) { printf("i%d ", y); }
  printf("| ");
  template for (auto x : mka(CW{1})) { printf("a%d ", x); }
  printf("| ");
  printf("%s", k.buf);
  printf("end\n");
}
