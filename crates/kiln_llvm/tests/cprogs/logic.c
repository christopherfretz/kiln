// Short-circuit operators (phi of i1) and the conditional operator.
int both(int a, int b) { return a && b; }
int either(int a, int b) { return a || b; }
int pick(int c, int a, int b) { return c ? a : b; }
