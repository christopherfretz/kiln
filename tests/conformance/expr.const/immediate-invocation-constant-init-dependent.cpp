// [expr.const.imm]/1.2, [expr.const.defns]/1.6: in a variable template
// whose initializer calls a consteval function with a value-dependent
// argument, each specialization's initializer is manifestly
// constant-evaluated and the call is validated by that specialization's
// constant-initialization check ([expr.const.init]/4), not on its own;
// the intermediate P's heap is freed at the end of the full-expression
// (spec/sema.md "Immediate invocations"; WP-549). Clang 22 and GCC 16.1
// accept this; clang 19 checks the instantiated call individually.
//@ expect: compile-ok
//@ requires: clang>=22
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
template <int N> consteval P flatn() { return flat(N); }

template <int N> constexpr D vd = flat(N).defect;
static_assert(vd<3>.a == 30);
static_assert(vd<0>.a == 0);

template <class T> constexpr D vs = flat(sizeof(T)).defect;
static_assert(vs<char>.a == 10);

template <int N> inline constexpr D vn = flatn<N>().defect;
static_assert(vn<2>.a == 20);

int main() { return vd<1>.a + vs<char>.a - 20; }
