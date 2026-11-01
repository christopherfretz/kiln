// [temp.inst]/4, [basic.scope.param]: a function template's body is
// instantiated with the definition's parameters, so a forward declaration
// that names a parameter `e` does not clash with a local `e` of a
// definition whose parameter is `e_in` (libc++ `__unicode` code-point
// views declared before their definitions). Verified against clang 19.
//@ expect: run
template <class I> constexpr int dist(I b, I e);
template <class I> constexpr int via(I b) { return dist(b, b + 1); }
template <class I> constexpr int dist(I b, I e_in) { I e = e_in; return int(e - b); }
constexpr char s[] = "ab";
static_assert(via(s + 0) == 1);
static_assert(dist(s + 0, s + 2) == 2);
int main() { return dist(s + 1, s + 2) == 1 ? 0 : 1; }
