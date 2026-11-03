// [dcl.ref]/5 (a reference is bound to a valid object or function, so it
// is never null), [basic.align]: clang 22's `EmitLoadOfReference` at
// -fclang-compat=22 (spec/codegen.md §3 "Reference loads"). The load of a
// reference's address from its slot carries `!nonnull !{}` unless the
// referee is incomplete, and `!align !{i64 A}` when the referee is an
// object whose (non-virtual, for a non-final class) alignment exceeds 1:
// local, parameter, global and member references, by-reference lambda
// captures, the range of a range-based for, and structured bindings.
// The module's memcpy declaration shows LLVM 21+'s `captures(none)`.
// Compared with clang 22's IR exactly.
//@ expect: ir
//@ requires: clang>=22
//@ requires: llvm>=22
struct Inc;
struct alignas(16) Over { int v; };
struct Base { virtual ~Base(); long l; };
struct Final final { double d; };
struct Holder { int &r; Over &o; };
struct Pair { int a, b; };

int g;
int &gr = g;
void fn();

Inc &incomplete(Inc &i) { return i; }
int local() { int x = 1; int &r = x; return r; }
int global() { return gr; }
char chr(char &c) { return c; }
void call(void (&f)()) { f(); }
int rvalue(int &&r) { return r; }
int over(Over &o) { return o.v; }
long base(Base &b) { return b.l; }
double final_class(Final &f) { return f.d; }
int member(Holder &h) { return h.r + h.o.v; }
int lambda(int &a) {
  auto l = [&a] { return a; };
  auto m = [&] { return a; };
  return l() + m();
}
int range(int (&arr)[3]) {
  int s = 0;
  for (int x : arr) s += x;
  return s;
}
int binding(Pair &p) {
  auto &[x, y] = p;
  return x + y;
}
void copy(Pair &d, Pair &s) { d = s; }
