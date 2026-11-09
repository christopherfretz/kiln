// [temp.inst], [stmt.expand]/5.3, [class.temporary]/9: each instantiation
// of a template, and each element's copy of a nested expansion statement,
// extends the temporaries of its own expansion-initializer, destroyed after
// its own last element. A non-dependent temporary (`D{1}`, `LW{&l, 'w'}`)
// is one node shared by the pattern and every instantiation; marking it
// lifetime-extended is the same decision at every use, so each use still
// creates and destroys its own object. A dependent one is rebuilt per
// instantiation. At run time and in constant evaluation. GCC 16.1 agrees;
// clang 22 has no P1306. WP-1149; spec/sema.md §7.6a.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ stdout: x1 x2 ~D1 | y3 y4 ~D2 | x1 x2 ~D1 | y3 y4 ~D3 | n10 n11 ~D6 ; n20 n21 ~D6 ; | 12w12a|12w12b|end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
struct P { int a, b; };
P mkp(const D&) { return {1, 2}; }
P mkq(const D&, int k) { return {k, k + 1}; }

template <class U> void tpl(U u) {
  template for (auto x : mkp(D{1})) printf("x%d ", x);
  printf("| ");
  template for (auto x : mkq(D{int(u)}, 3)) printf("y%d ", x);
  printf("| ");
}

// The inner statement is instantiated once per outer element.
void nested() {
  template for (auto i : {10, 20}) {
    template for (auto x : mkq(D{6}, i)) printf("n%d ", x);
    printf("; ");
  }
  printf("| ");
}

struct L {
  char buf[16]{};
  int n = 0;
  constexpr void put(char c) { buf[n++] = c; }
};
struct LW {
  L* l;
  char c;
  constexpr ~LW() { l->put(c); }
};
constexpr P lp(const LW&) { return {1, 2}; }
template <class U> constexpr L crun(U c) {
  L l;
  template for (auto x : lp(LW{&l, 'w'})) l.put(char('0' + x));
  template for (auto x : lp(LW{&l, char(c)})) l.put(char('0' + x));
  l.put('|');
  return l;
}
constexpr L k1 = crun('a');
constexpr L k2 = crun(int('b'));

int main() {
  tpl(2);
  tpl(3L);
  nested();
  printf("%s%send\n", k1.buf, k2.buf);
}
