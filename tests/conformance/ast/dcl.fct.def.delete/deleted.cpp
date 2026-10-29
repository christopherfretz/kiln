//@ expect: ast
void nope(int) = delete;
void nope(long) {}
inline int in(int x) { return x; }
static int st() { return 0; }
extern int ex();
int call() { return in(st()) + ex(); }
