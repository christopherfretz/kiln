// [class.conv.fct], [over.ics.user]: a conversion function returning a
// reference (`operator bool&()`) used where a prvalue of the referenced
// type is needed is the call, an lvalue, then the lvalue-to-rvalue
// conversion: the referee's value is loaded, not its address passed
// (libc++ test_execution_policies.h `Bool`, pstl.for_each.pass.cpp).
// Matches clang 19.
//@ expect: run
//@ stdout: 1 1 0 5
extern "C" int printf(const char*, ...);
struct Bool {
  bool b_;
  operator bool&() { return b_; }
  operator const bool&() const { return b_; }
};
struct Num {
  int n;
  operator int&() { return n; }
};
bool id(bool b) { return b; }
int twice(int x) { return 2 * x; }
template <class P> bool call(P p, Bool& x) { return p(x); }
int main() {
  Bool t{true};
  const Bool f{false};
  bool r = id(t);
  bool s = call([](bool b) { return b; }, t);
  Num k{2};
  int v = twice(k) + 1;
  printf("%d %d %d %d\n", r, s, id(f), v);
}
