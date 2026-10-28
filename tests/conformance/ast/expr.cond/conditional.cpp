//@ expect: ast
int g;
int &ref(bool b, int &x) { return b ? x : g; }
long widen(bool b, int i, long l) { return b ? i : l; }
const char *msg(bool b) { return b ? "yes" : nullptr; }
void side(bool b) { b ? (void)0 : (void)g; }
