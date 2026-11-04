// [expr.const.imm]/1.2, [expr.const.defns]/1.6: the initializer of a
// static constexpr data member is manifestly constant-evaluated, so the
// consteval call inside it is validated by the member's
// constant-initialization check ([expr.const.init]/4), not on its own;
// the heap owned by the intermediate P is freed at the end of the
// full-expression and only D becomes the member's value (spec/sema.md
// "Immediate invocations"; WP-549). GCC 16.1 accepts this; clang 19 and
// 22 reject the in-class initializer (they check the call individually),
// so GCC is the oracle.
//@ expect: compile-ok
//@ skip(clang): clang checks a consteval call in an in-class static member initializer individually
//@ oracle: gcc
//@ strict-diags
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

struct S { static constexpr D sd = flat(2).defect; };
static_assert(S::sd.a == 20);

template <class T> struct TS { static constexpr D sd = flat(sizeof(T)).defect; };
static_assert(TS<char>::sd.a == 10);

int main() { return S::sd.a - 20; }
