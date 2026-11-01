// [basic.start.static], [basic.start.dynamic]: a global whose initializer
// is not a constant expression is zero-initialized and then initialized
// by an internal __cxx_global_var_init function, run in declaration
// order from _GLOBAL__sub_I_<file> (llvm.global_ctors priority 65535);
// constant initializers keep the constant path.
int f();
struct A { A(); A(int); int x; };
struct C { constexpr C(int v) : x(v) {} int x; };
A a1;
int g1 = f();
A a2(3);
namespace { A anon; }
static A sa(4);
int *p = new int(3);
const int ci = f();
int cg = 7;
int &cr = cg;
const C cc(5);
int g2 = g1 + 1;
int use() { return anon.x + sa.x + ci; }
const C *usec() { return &cc; }
