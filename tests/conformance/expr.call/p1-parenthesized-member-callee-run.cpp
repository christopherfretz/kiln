//@ expect: run
//@ stdout: 6 3 5 8 9 2 2 2 1
// [expr.call]/1-2: the postfix expression of a member function call may be
// parenthesized, `(o.f)()`, `(p->f)()`, `((o.*pm))()`; the implied object
// argument is found through the parentheses, in constant evaluation, in
// warning analysis and at run time, also for virtual dispatch and
// instantiated templates.
#include <cstdio>

struct S {
  int v;
  constexpr int get() const { return v; }
  int add(int a) { return v += a; }
};
struct B { virtual int id() const { return 1; } virtual ~B() = default; };
struct D : B { int id() const override { return 2; } };

constexpr S cs{5};
static_assert((cs.get)() + 1 == 6, "");
static_assert(((cs.get))() - 1 == 4, "");

template <class T> int twice(T& t) { return (t.add)(t.v); }
int f() noexcept(noexcept((cs.get)())) { return 0; }

int main() {
  S s{5};
  S* p = &s;
  int (S::*pm)(int) = &S::add;
  int r = 0;
  for (int i = 0; (cs.get)() - i > 1; ++i) r = i;
  D d;
  const B& b = d;
  std::printf("%d %d %d ", (cs.get)() + 1, r, (s.get)());
  std::printf("%d %d ", (p->add)(3), ((s.*pm))(1));
  std::printf("%d ", (b.id)());
  S t{1};
  int tw = twice(t) + f();
  std::printf("%d %d %d\n", tw, (t.get)(), (b.B::id)());
}
