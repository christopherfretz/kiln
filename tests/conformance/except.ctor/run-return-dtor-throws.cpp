// [except.ctor]/2 (CWG2176): when a destructor of a temporary or of a
// local variable throws after a return statement has initialized the
// returned object, the returned object is destroyed, once: a prvalue
// result, the right operand of a comma whose left temporary throws, and a
// named return value with a throwing local declared before it (one
// declared after it is destroyed while the named value's own cleanup
// still covers it). Found running GCC's C++ testsuite.
//@ xfail(clang): clang 19 and 22 leak the returned object
//@ expect: run
//@ oracle: gcc
//@ stdout: f 0 | g 0 | n1 0 | n2 0 | h 0
extern "C" int printf(const char*, ...);
int live;
struct X {
  bool t;
  X(bool t) : t(t) { ++live; }
  X(const X&);
  ~X() noexcept(false) {
    --live;
    if (t) throw 1;
  }
};
X f() { X x(true); return X(false); }
X g() { return X(true), X(false); }
X n1() { X x(true); X r(false); return r; }
X n2() { X r(false); X x(true); return r; }
X h() {
  try { X x(true); return X(false); } catch (...) {}
  return X(false);
}
#define RUN(fn) live = 0; try { fn(); } catch (...) {} printf(#fn " %d | ", live);
int main() {
  RUN(f) RUN(g) RUN(n1) RUN(n2)
  live = 0;
  { X r = h(); }
  printf("h %d\n", live);
}
