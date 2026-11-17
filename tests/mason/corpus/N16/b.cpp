#define PLUS +
inline int f(int a) { if (a) { return a PLUS 1; } return 0; }
int (*keep_b)(int) = f;
