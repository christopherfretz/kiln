// Debug info (WP-910): a namespace-scope structured binding's variable
// is a DIGlobalVariable with no name, only its linkage name
// ([dcl.struct.bind]); LLVM rejects an empty `name:`.
//@ args: -g
struct P { int x; long y; };
constexpr P p1{1, 2}, p2{5, 6};
auto [a, b] = p1;
int arr[2] = {3, 4};
auto &[c, d] = arr;
static auto [e, f] = p2;
long use() { return a + b + c + d + e + f; }
