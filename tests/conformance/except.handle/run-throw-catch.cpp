// [except.throw]/1-3, [except.handle]/3-4,7, [except.handle]/14 (rethrow):
// thrown objects reach the first matching handler (by value, reference,
// pointer, base class, catch-all), `throw;` rethrows the current
// exception, and handlers of nested try-blocks are searched outward.
//@ expect: run
//@ strict-diags
struct Base { int code; Base(int c) : code(c) {} virtual ~Base() {} };
struct Derived : Base { Derived(int c) : Base(c) {} };
int copies = 0;
struct Counted {
  int v;
  Counted(int x) : v(x) {}
  Counted(const Counted& o) : v(o.v) { ++copies; }
};

int by_value() { try { throw 7; } catch (int x) { return x; } return 0; }
int by_ref() { try { throw 8; } catch (int& x) { x += 1; return x; } return 0; }
int by_base() {
  try { throw Derived(10); } catch (const Base& b) { return b.code; }
  return 0;
}
int by_pointer() {
  Derived d(11);
  try { throw &d; } catch (Base* b) { return b->code; }
  return 0;
}
int order() {
  try { throw 2.5; }
  catch (int) { return 1; }
  catch (double d) { return d == 2.5 ? 12 : 2; }
  catch (...) { return 3; }
  return 0;
}
int catch_all() { try { throw Counted(1); } catch (...) { return 13; } return 0; }
int class_value() {
  try { throw Counted(14); } catch (Counted c) { return c.v; }
  return 0;
}
int rethrow() {
  try {
    try { throw 15; } catch (int) { throw; }
  } catch (int x) { return x; }
  return 0;
}
int nested_outward() {
  try {
    try { throw Derived(16); } catch (int) { return 1; }
  } catch (Base& b) { return b.code; }
  return 0;
}
int thrower(int k) { if (k > 0) throw k; return 0; }
int deep(int k) { return k == 0 ? thrower(17) : deep(k - 1); }
int through_frames() { try { deep(5); } catch (int x) { return x; } return 0; }
int conditional(bool b) {
  try { int r = b ? throw 18 : 1; return r; } catch (int x) { return x; }
}

int main() {
  if (by_value() != 7) return 1;
  if (by_ref() != 9) return 2;
  if (by_base() != 10) return 3;
  if (by_pointer() != 11) return 4;
  if (order() != 12) return 5;
  if (catch_all() != 13) return 6;
  if (class_value() != 14) return 7;
  if (rethrow() != 15) return 8;
  if (nested_outward() != 16) return 9;
  if (through_frames() != 17) return 10;
  if (conditional(true) != 18) return 11;
  if (conditional(false) != 1) return 12;
  return 0;
}
