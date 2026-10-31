// [except.pre]/4, [except.handle]/14-15: function-try-blocks of
// functions, constructors (implicit rethrow) and destructors; handler
// locals and returns from try-blocks.
void f();
int g(int);
struct S { int a; S(); S(const S&); ~S(); };
struct M { M(); ~M(); };
struct P { P(int); ~P(); };

int ft(int x) try { return g(x); } catch (int e) { return e; } catch (...) { return -1; }

struct C : P {
  M m;
  S s;
  C() try : P(g(1)), m(), s() { f(); } catch (...) { g(2); }
  ~C() try { f(); } catch (...) { g(3); }
};
void use() { C c; }

struct V {
  M m;
  ~V() try { f(); } catch (int) { g(4); return; } catch (...) {}
};
void usev() { V v; }

void handler_locals() {
  try { f(); } catch (S s) { S t; g(s.a + t.a); }
}
void try_in_handler() {
  try { f(); } catch (int) { try { f(); } catch (...) { g(1); } }
}
int ret_in_try() {
  S s;
  try { S t; return g(1); } catch (...) { return 2; }
}
void throw_in_try() {
  try { throw 1; } catch (int x) { g(x); }
}
void throw_class_in_try() {
  try { throw S(); } catch (S &s) { g(s.a); }
}
