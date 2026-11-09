// [class.temporary]/8 (CWG3043, C++26 DIS), [expr.const]: in constant
// evaluation an enumerating expansion statement's element temporaries
// persist for the lifetime of the for-range-declaration and are destroyed
// after it, in reverse order of construction, at the end of the element's
// block, as at run time (element-temporaries-run.cpp). The log records
// each constexpr destructor; the same function runs at compile time (the
// constexpr variable, and the static_assert) and at run time, and both logs
// must match. A temporary destroyed before its element's body ran would log
// its letter before the digit. GCC 16.1 implements the earlier resolution
// (`a0b1`, and `E{...}.self()` dangles: not a constant expression); clang 22
// has no P1306. WP-1106; spec/sema.md §7.6a.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: none
//@ stdout: 0a1b|2c3d|4e5f|1hg|3d|7i9j|end
//@ stdout: 0a1b|2c3d|4e5f|1hg|3d|7i9j|end
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
  constexpr int v() const { return c - 'a'; }
  constexpr const E& self() const { return *this; }
};
constexpr E mk(L* l, char c) { return E{l, c}; }
struct C {
  int v;
  constexpr ~C() {}
};
template <class T> constexpr void dep(L& l, T i) {
  template for (auto x : {E{&l, char('i' + i)}.v() - 1, E{&l, char('j' + i)}.v()}) {
    l.put(char('0' + x));
  }
}
constexpr L run() {
  L l;
  // By value, two elements.
  template for (auto x : {E{&l, 'a'}.v(), E{&l, 'b'}.v()}) l.put(char('0' + x));
  l.put('|');
  // const auto&, bound directly.
  template for (const auto& x : {E{&l, 'c'}, E{&l, 'd'}}) l.put(char('0' + x.v()));
  l.put('|');
  // auto&&, a prvalue and a returned prvalue.
  template for (auto&& x : {E{&l, 'e'}, mk(&l, 'f')}) l.put(char('0' + x.v()));
  l.put('|');
  // Two temporaries in one element: reverse order of construction.
  template for (int x : {E{&l, 'g'}.v() && E{&l, 'h'}.v()}) l.put(char('0' + x));
  l.put('|');
  // A reference through a member call does not dangle.
  template for (const auto& x : {E{&l, 'd'}.self()}) l.put(char('0' + x.v()));
  l.put('|');
  dep(l, 0);
  l.put('|');
  return l;
}
constexpr L k = run();
static_assert(k.buf[0] == '0' && k.buf[1] == 'a' && k.buf[2] == '1' && k.buf[3] == 'b');
int main() {
  // A constexpr or const for-range-declaration whose element extends a
  // temporary with a constexpr destructor is still constant-initialized.
  template for (constexpr int x : {C{3}.v, 5}) static_assert(x == 3 || x == 5);
  template for (const int x : {C{6}.v}) static_assert(x == 6);
  printf("%send\n", k.buf);
  L l = run();
  printf("%send\n", l.buf);
}
