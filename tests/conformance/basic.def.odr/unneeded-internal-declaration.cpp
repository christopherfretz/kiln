// [basic.def.odr]/5 (WP-234): a namespace-scope variable with internal
// linkage that is referenced but never odr-used (only in unevaluated
// operands or an uninstantiated template) is not needed
// (-Wunneeded-internal-declaration, in -Wunused-function and
// -Wunused-template); one never referenced is -Wunused-variable. A
// referenced variable usable in constant expressions is left alone.
//@ args: -Wunused
//@ strict-diags
//@ diag: 20:12 warning "variable 'a' is not needed and will not be emitted"
//@ diag: 28:17 warning "unused variable 'e'"
//@ diag: 28:28 warning "variable 'f' is not needed and will not be emitted"
//@ diag: 30:12 warning "variable 'g' is not needed and will not be emitted"
//@ diag: 36:12 warning "variable 'j' is not needed and will not be emitted"
//@ diag: 39:13 warning "unused variable 'ref'"
//@ diag: 40:12 warning "variable 'arr' is not needed and will not be emitted"
//@ diag: 42:12 warning "variable 'y2' is not needed and will not be emitted"
//@ diag: 45:12 warning "unused variable 'z3'"
//@ diag: 47:25 warning "unused variable 'ec'"
//@ diag: 48:35 warning "unused variable 'deep'"
static int a = 1;
int ua() { return sizeof(a); }
static int b = 2;
int ub() { return b; }
static const int c = 3;
int uc() { return c; }
static const int d = 4;
const int *ud() { return &d; }
namespace { int e = 5; int f = 6; }
decltype(f) uf() { return 0; }
static int g = 7;
int ug() { return sizeof g + alignof(decltype(g)); }
static constexpr int h = 8;
int uh() { return h; }
static int i2 = 9;
void ui() { (void)i2; }
static int j = 10;
template <class T> int uj() { return j; }
static int x;
static int &ref = x;
static int arr[3];
int uarr() { return sizeof(arr) / sizeof(arr[0]); }
static int y2; int uy2 = sizeof(y2);
static const int ci = 5;
decltype(ci) uci();
static int z3 = 0, z4 = 1;
int uz() { return z4; }
extern "C" { static int ec; }
namespace { namespace inner { int deep; } }
static int w [[maybe_unused]];
