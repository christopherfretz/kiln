// [expr.type.conv]/2: a cv-qualified array prvalue `CA{...}` keeps its
// cv-qualifiers ([expr.type]/2); only non-class non-array prvalues drop them.
//@ expect: ast
using CA = const int[2];
int f() { return CA{1, 2}[1]; }
struct D { ~D(); };
using CD = const D[2];
void g() { (void)CD{}; }
constexpr int h() { return CA{3, 4}[0]; }
static_assert(h() == 3);
