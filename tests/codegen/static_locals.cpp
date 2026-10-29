// Static locals with constant initializers and a counter.
int counter() { static int n = 0; return ++n; }
const char* name() { static const char* s = "name"; return s; }
int table(int i) { static int t[4] = {10, 20, 30, 40}; return t[i & 3]; }
int statics() { return counter() + counter() + name()[0] + table(5); }
