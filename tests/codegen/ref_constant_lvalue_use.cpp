// [basic.def.odr]/4, [expr.const]/3: a reference usable in constant
// expressions, named as an lvalue (`e.y`, `h[i]` with a run-time index), is
// not an odr-use and is emitted as its referee's address (clang
// `EmitDeclRefLValue`, `NOUR_Constant`), with no load of the reference's
// slot; a reference bound to a lifetime-extended class or array temporary
// at namespace scope is loaded as clang loads it.
struct E { int x, y; };
constexpr E ee{1, 2};
constexpr const E &e = ee;
const int *py() { return &e.y; }
int arr[3];
constexpr int (&h)[3] = arr;
int hi(int i) { return h[i]; }
struct T { int a; };
const T &tr = T{5};
int tri() { return tr.a; }
const int (&ar)[2] = {7, 8};
int ari(int i) { return ar[i]; }
