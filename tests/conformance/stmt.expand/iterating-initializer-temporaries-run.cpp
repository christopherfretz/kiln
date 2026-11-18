// [class.temporary]/9, [stmt.expand]/5.2: the temporaries created in the
// expansion-initializer of an iterating expansion statement persist for
// the lifetime of the reference it initializes, `decltype(auto) range =
// (init);` when `init` is a glvalue, and so end with the statement: after
// the last element's body, before the next statement (kiln destroyed them
// at the end of the enclosing block: the initializer, built before the
// declaration, had no full-expression of its own). A prvalue initializes
// the object `range` itself, so there is no reference and by the text its
// temporaries end with that declaration; kiln keeps them to the end of the
// statement, as GCC 16.1 does (docs/status/deviations.md). A by-value
// class argument of a glvalue range's call is a parameter, not a
// temporary: kiln ends it with the initializer's full-expression, as
// clang 22 does in a range-based for ([expr.call]/7,
// implementation-defined; GCC 16.1 extends it).
// At run time, in a template, and in constant evaluation. Found running
// GCC's C++ testsuite; clang 22 has no P1306. WP-1694; spec/sema.md §7.6a.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: none
//@ stdout: a5 a6 ~D1 | b7 b8 ~D2 | ~D3 c5 c6 | t5 t6 ~D10 | t5 t6 ~D20 | 56x|end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
struct R {
  int v[2];
  constexpr const int* begin() const { return v; }
  constexpr const int* end() const { return v + 2; }
};
R r0{{5, 6}};
const R& ref(const D&) { return r0; }
R val(const D&) { return R{{7, 8}}; }
const R& byval(D) { return r0; }

template <class U> void tpl(U u) {
  template for (auto x : ref(D{u})) printf("t%d ", x);
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
constexpr R k0{{5, 6}};
constexpr const R& cref(const LW&) { return k0; }
constexpr L run() {
  L l;
  template for (auto x : cref(LW{&l, 'x'})) l.put(char('0' + x));
  l.put('|');
  return l;
}
constexpr L k = run();
static_assert(k.buf[2] == 'x');

int main() {
  template for (auto x : ref(D{1})) printf("a%d ", x);
  printf("| ");
  template for (auto x : val(D{2})) printf("b%d ", x);
  printf("| ");
  template for (auto x : byval(D{3})) printf("c%d ", x);
  printf("| ");
  tpl(10);
  tpl(20);
  printf("%send\n", k.buf);
}
