// sides: pre post
// scope: any
// Closures, local classes and unnamed types that a unit numbers: an
// unrelated one of each before or after the definitions.
inline auto kc_lambda = [] { return 1; };
static int kc_fn() { struct Q { int a; }; auto l = [](int x) { return x; }; return Q{1}.a + l(2); }
struct { int a; } kc_unnamed_var;
enum { KC_E = 1 };
typedef struct { int b; } KcTypedefUnnamed;
inline int kc_inl() { auto a = [] { return 2; }; struct R { int r; }; return a() + R{3}.r + kc_fn() + kc_unnamed_var.a; }
namespace { int kc_anon() { return 3; } struct KcAnon { int z; }; }
inline int kc_use_anon() { return kc_anon() + KcAnon{1}.z; }
