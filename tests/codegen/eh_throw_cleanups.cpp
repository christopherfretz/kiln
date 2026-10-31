// [expr.throw], [except.terminate]: throws inside conditional
// branches, under temporaries and locals, and in a noexcept function
// (terminate landing pad).
struct S { int a; S(int); S(const S&); ~S(); };
struct T { T(); ~T(); int v() const; };
int g(int);
void h1(int c) { c ? throw S(T().v()) : (void)g(1); }
void h3(int c) { g(c && (throw S(1), true)); }
void h4() { throw S(T().v()); }
int h5(int c) { T t; if (c) throw S(c); return t.v(); }
void h6(int c) { int x = c ? g(1) : throw S(g(2)); g(x); }
void h8(int c) { (c ? throw 1 : throw 2); }
void h9() { for (;;) { T t; if (g(0)) throw 3; } }
int k(int c) { return c ? T().v() : 0; }
