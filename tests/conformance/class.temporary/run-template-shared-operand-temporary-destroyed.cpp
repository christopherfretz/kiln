// [class.temporary]/4, [intro.execution]/5, [temp.inst]: in an instantiated
// function template, a temporary in a non-dependent operand of a rebuilt
// full-expression is still destroyed at the end of that full-expression.
// Examples are `B b(S{2});` and a dependent call `use(t, S{1})`. The
// instantiation reuses the operand unchanged, but its temporaries still
// belong to the full-expression (libc++ any emplace.pass.cpp:
// `std::any a(small{42});` in a test template).
//@ expect: run
//@ strict-diags
//@ stdout: 0 1 2 2 2 2 | 4 0
extern "C" int printf(const char *, ...);
int live = 0;
struct S {
  S(int) { ++live; }
  S(const S &) { ++live; }
  ~S() { --live; }
  int v() const { return 1; }
};
struct B {
  B(const S &) { ++live; }
  ~B() { --live; }
};
struct A {
  template <class V> A(V &&) { ++live; }
  ~A() { --live; }
};
template <class T> void use(T, const S &) {}
int plain(const S &s) { return s.v(); }

template <class T> int tt(T t) {
  use(t, S{1}); printf("%d ", live);
  B b(S{2}); printf("%d ", live);
  A a(S{3}); printf("%d ", live);
  int r = plain(S{4}) + t; printf("%d ", live);
  if (plain(S{5}) + t) printf("%d ", live);
  t = plain(S{6}); printf("%d |", live);
  return plain(S{7}) + t + r;
}

// Constant evaluation of an instantiation: the destructor runs at the end
// of the declaration, before the next statement reads the counter.
struct C {
  int *p;
  constexpr C(int *q) : p(q) {}
  constexpr ~C() { ++*p; }
};
struct D {
  constexpr D(const C &) {}
};
template <class T> constexpr T probe() {
  T n = 0;
  D d(C{&n});
  return n;
}
static_assert(probe<int>() == 1);

int main() {
  int r = tt(1);
  printf(" %d %d\n", r, live);
}
