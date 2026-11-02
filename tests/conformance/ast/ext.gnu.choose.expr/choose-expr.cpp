// __builtin_choose_expr (GNU): the selected operand's type and value
// category; the constant condition and both operands are children (WP-316).
int a = 1;
long b = 2;
int x = __builtin_choose_expr(1, a, b);
long y = __builtin_choose_expr(sizeof(int) == 8, a, b);
int &r = __builtin_choose_expr(true, a, 0.5);
constexpr int k = __builtin_choose_expr(0, 1.5, 7) + 1;
template <class T> int f(T t) { return __builtin_choose_expr(sizeof(T) == 4, t, 0); }
int g() { return __builtin_choose_expr(false, a, x); }
