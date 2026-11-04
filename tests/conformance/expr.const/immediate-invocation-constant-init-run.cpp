// [expr.const.imm]/1.2, [expr.const.defns]/1.6, [expr.const.init]/4: a
// consteval call inside a constant-initialized variable's initializer
// is validated by the variable's check, not on its own; only the
// trivially-copyable D member of the heap-owning P becomes the value.
// The values are statically initialized and read (and one is modified)
// at run time (spec/sema.md "Immediate invocations"; WP-549).
//@ expect: run
//@ strict-diags
//@ stdout: 10 20 30 40 50
//@ exit: 0
extern "C" int printf(const char*, ...);
struct Buf {
  int* p = nullptr;
  int n = 0;
  constexpr Buf() = default;
  constexpr Buf(const Buf& o) : p(new int[o.n > 0 ? o.n : 1]), n(o.n) {
    for (int i = 0; i < n; ++i) p[i] = o.p[i];
  }
  Buf& operator=(const Buf&) = delete;
  constexpr ~Buf() { delete[] p; }
  constexpr void push(int v) {
    int* q = new int[n + 1];
    for (int i = 0; i < n; ++i) q[i] = p[i];
    q[n++] = v;
    delete[] p;
    p = q;
  }
};
struct D { int a = 0; bool bad = false; };
struct P { Buf entries; D defect{}; };
consteval P flat(int k) {
  P f;
  for (int i = 0; i < k; ++i) f.entries.push(i);
  f.defect.a = f.entries.n * 10;
  return f;
}

constexpr D e1 = flat(1).defect;
constinit D ci = flat(2).defect;
constexpr int fn() { constexpr D d = flat(3).defect; return d.a; }
template <class T> constexpr D vt = flat(4).defect;
// Clang 19 checks the call on its own when the specialization is first
// instantiated from a non-constant context; instantiate it here first.
static_assert(vt<char>.a == 40);
D gs = flat(5).defect;

int main() {
  printf("%d %d %d %d %d\n", e1.a, ci.a, fn(), vt<char>.a, gs.a);
  ci.a += gs.a;
  return ci.a == 70 && !vt<char>.bad ? 0 : 1;
}
