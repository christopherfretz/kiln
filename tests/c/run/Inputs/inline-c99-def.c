inline int sq(int x) { return x * x; }
extern int sq(int x);
int other(void) { return sq(1); }
