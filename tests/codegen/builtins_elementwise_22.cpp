//@ min-oracle: 22
// WP-416: clang 22 constant-evaluates __builtin_elementwise_abs and the
// integer __builtin_elementwise_max / _min (clang 19 does not), so
// `EmitBuiltinExpr` keeps the folded value.

int k1() { return __builtin_elementwise_max(2, 5); }
unsigned k2() { return __builtin_elementwise_min(7u, 5u); }
float k3() { return __builtin_elementwise_abs(-2.0f); }
int k4() { return __builtin_elementwise_abs(-9); }
