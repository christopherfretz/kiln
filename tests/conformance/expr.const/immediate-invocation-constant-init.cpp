// [expr.const.imm]/1.2, [expr.const.defns]/1.6: the initializer of a
// variable that is usable in constant expressions or has constant
// initialization is manifestly constant-evaluated, so a consteval call
// inside it is in an immediate function context and is validated by the
// variable's constant-initialization check ([expr.const.init]/4), not as
// an immediate invocation of its own. The consteval call returns a P
// whose Buf member owns heap; the Buf is destroyed at the end of the
// full-expression and only the D member becomes the variable's value, so
// each initializer is a constant expression (spec/sema.md "Immediate
// invocations"; WP-549). Covers namespace-scope constexpr, constinit, a
// local constexpr variable in a constexpr function, a non-dependent
// variable template, and non-constexpr variables with static storage
// duration at namespace and block scope (their initializers are constant
// initializers, [basic.start.static]/2). Matches clang 19 and 22 and GCC
// 16.1.
//@ expect: compile-ok
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

constexpr D e1 = flat(1).defect;
static_assert(e1.a == 10 && !e1.bad);

constinit D ci = flat(2).defect;

constexpr int fn() { constexpr D d = flat(3).defect; return d.a; }
static_assert(fn() == 30);

template <class T> constexpr D vt = flat(4).defect;
static_assert(vt<int>.a == 40);
static_assert(vt<char>.a == 40);

D gs = flat(5).defect;

int loc() { static D s = flat(6).defect; return s.a; }

int main() { return ci.a + gs.a + loc() - 130; }
