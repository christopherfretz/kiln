// [class.temporary]/8 (CWG3043, C++26 DIS), [expr.context]/2,
// [expr.static.cast]/6, [expr.const]: in constant evaluation a discarded
// class prvalue of an enumerating expansion-init-list element is
// materialized and persists for the lifetime of the for-range-declaration,
// as at run time (element-discarded-temporary-run.cpp). The log records
// each constexpr destructor; the same function runs at compile time (the
// constexpr variable, and the static_assert) and at run time, and both
// logs must match: the element's digit comes before its letters, several
// letters in reverse order of construction, and an ordinary declaration's
// discarded temporary (`g`) dies before its value is logged. A constexpr
// for-range-declaration whose element extends such a temporary is still
// constant-initialized. GCC 16.1 extends no element temporary; clang 22
// has no P1306. WP-1117; spec/sema.md §7.6a.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: none
//@ stdout: 1a|2b3c|4ed|5f|g6end
//@ stdout: 1a|2b3c|4ed|5f|g6end
extern "C" int printf(const char*, ...);
struct L {
  char buf[64]{};
  int n = 0;
  constexpr void put(char c) { buf[n++] = c; }
};
struct E {
  L* l;
  char c;
  constexpr ~E() { l->put(c); }
};
constexpr E mk(L* l, char c) { return E{l, c}; }
struct C {
  int v;
  constexpr ~C() {}
};
template <class T> constexpr void dep(L& l) {
  template for (int x : {((void)T{&l, 'f'}, 5)}) l.put(char('0' + x));
}
constexpr L run() {
  L l;
  template for (int x : {((void)E{&l, 'a'}, 1)}) l.put(char('0' + x));
  l.put('|');
  template for (int x : {(mk(&l, 'b'), 2), (void(E{&l, 'c'}), 3)}) l.put(char('0' + x));
  l.put('|');
  template for (int x : {((void)E{&l, 'd'}, (void)mk(&l, 'e'), 4)}) l.put(char('0' + x));
  l.put('|');
  dep<E>(l);
  l.put('|');
  int y = ((void)E{&l, 'g'}, 6);
  l.put(char('0' + y));
  return l;
}
constexpr L k = run();
static_assert(__builtin_strcmp(k.buf, "1a|2b3c|4ed|5f|g6") == 0);
int main() {
  template for (constexpr int x : {((void)C{1}, 7), ((void)C{2}, (void)C{3}, 8)}) {
    static_assert(x == 7 || x == 8);
  }
  printf("%send\n", k.buf);
  L l = run();
  printf("%send\n", l.buf);
}
