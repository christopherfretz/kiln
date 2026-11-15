// [class.temporary]/6, [expr.prim.this], [class.mem.general]: `this` in a
// default member initializer of a temporary bound to a `constexpr`
// reference names the lifetime-extended object itself, so `r.p == &r`,
// at compile time and at run time, also through a nested member and for
// a static local and a `thread_local` reference. kiln's constant emitter
// recursed into the temporary's own address and overflowed the stack, and
// two self-pointers made the constant walk exponential. Found running
// GCC's C++ testsuite (WP-1456).
//@ expect: run
//@ strict-diags
//@ exit: 0
struct X { const X* p = this; };
struct In { const void* q; };
struct Y { int a = 1; In in = {this}; const Y* self = this; };
constexpr const X& r = X{};
constexpr const Y& ry = Y{};
static_assert(r.p == &r, "");
static_assert(ry.in.q == &ry && ry.self == &ry, "");
const X& f() { static constexpr const X& s = X{}; return s; }
thread_local const X& t = X{};
int main() {
  if (r.p != &r) return 1;
  if (ry.in.q != &ry || ry.self != &ry) return 2;
  if (f().p != &f()) return 3;
  if (t.p != &t) return 4;
  return 0;
}
