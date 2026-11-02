// [dcl.fct]/22, [temp.inst]: a parameter declared before the first
// placeholder parameter (`int c` in `f(int c, auto& x)`) belongs to the
// abbreviated function template and is instantiated with it, in a member
// (libc++ formatter.handle.pass.cpp `format(color c, auto& ctx)`), a free
// function and an out-of-line member definition.
const char* names[] = {"black", "red", "gold"};
struct B { int format(const char* p, int& ctx) const { return *p + ctx; } };
struct F : B {
  auto format(int c, auto& ctx) const { return B::format(names[c], ctx); }
  int twice(int c, auto& ctx) const;
};
int F::twice(int c, auto& ctx) const { return c + c + ctx; }
int add(int c, short d, auto& x) { return c + d + x; }
int use() {
  F f;
  int x = 1;
  long y = 2;
  return f.format(1, x) + f.twice(2, y) + add(3, 4, x);
}
