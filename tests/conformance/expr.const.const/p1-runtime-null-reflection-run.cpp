// [expr.const.const]/1.1 (P4101R1): the null reflection is not a
// consteval-only value, so an object of type `std::meta::info` (or of a
// class holding one) holding it exists at run time, in a non-constexpr
// variable, a parameter, a return value, a heap object and a member.
// kiln represents it as a pointer-sized null (spec/codegen.md "Run-time
// reflections"): zero-initialization, value-initialization, copy,
// assignment, `==`/`!=` and passing/returning by value (directly, through
// a function pointer, a member pointer, a virtual call and a lambda, and
// as a deduced template argument) all work, and functions whose type
// holds `info` need not be `consteval` (/1, [expr.const.imm]/3: nothing
// in them is an immediate constant expression). kiln only: GCC 16.1
// rejects every declaration here under the P2996R13 type rule
// (docs/status/deviations.md).
//@ expect: run
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ stdout: 8 8 1 0 1 1 1 1 1 1 1 1 1 7
extern "C" int printf(const char*, ...);
using info = decltype(^^int);
struct S { info r; int n; };
struct Holder { info a[2]; S s; };

info g;            // zero-initialized: the null reflection
S gs;
Holder gh;
static info sg = info();

info id(info x) { return x; }
S ids(S s) { s.n += 1; return s; }
Holder idh(Holder h) { return h; }
bool eq(info a, info b) { return a == b; }
bool ne(info a, info b) { return a != b; }
info pick(int, info a, double, info b) { return a == b ? a : b; }

template <class T> T tid(T t) { return t; }

struct Base {
  virtual info get(info x) const { return x; }
  virtual ~Base() = default;
};
struct Derived : Base {
  info get(info x) const override { return id(x); }
};

struct M {
  info r;
  info get() const { return r; }
};

int main() {
  info x{};
  info y = x;
  y = g;
  info* p = new info();
  info (*fp)(info) = &id;
  info (M::*mp)() const = &M::get;
  M m{};
  Derived d;
  const Base& b = d;
  auto lam = [](info v, S s) { return v == s.r; };
  S s{};
  S t = ids(s);
  Holder h = idh(gh);
  static info sl;
  printf("%zu %zu %d %d %d %d %d %d %d %d %d %d %d %d\n",
         sizeof(info), alignof(info),
         eq(id(x), g), ne(x, y), y == info{}, *p == sg,
         fp(x) == (m.*mp)(), b.get(y) == tid(sl), lam(x, t),
         t.r == gs.r, h.a[1] == h.s.r, pick(1, x, 2.0, y) == info(),
         tid(t).n == 1, 7);
  delete p;
  return 0;
}
