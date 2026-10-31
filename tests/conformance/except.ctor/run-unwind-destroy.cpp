// [except.ctor]/1-4: automatic objects are destroyed during stack
// unwinding in reverse order of construction; a partially constructed
// object has its constructed bases and members destroyed (also array
// elements and objects of a delegating constructor); a constructor's
// function-try-block handler rethrows when it falls off ([except.handle]/14).
// The same holds for the members and elements of a braced aggregate
// initialization ([dcl.init.aggr]/7).
//@ expect: run
//@ strict-diags
int log_[32];
int n = 0;
void note(int v) { log_[n++] = v; }
int may_throw(int v) { if (v < 0) throw v; return v; }

struct L { int id; L(int i) : id(i) { may_throw(i); note(i); } ~L() { note(-id - 100); } };

int locals() {
  try {
    L a(1);
    L b(2);
    may_throw(-1);
  } catch (int) {
    return 0;
  }
  return 1;
}

struct B { B() { note(10); } ~B() { note(-10); } };
struct M { M(int v) { may_throw(v); note(11); } ~M() { note(-11); } };
struct C : B {
  M m1;
  M m2;
  C(int v) : m1(1), m2(v) { note(99); }
  ~C() { note(-99); }
};
struct D {
  M m;
  D() : m(1) {}
  D(int v) : D() { may_throw(v); }
};
struct E { L arr[3]; E() : arr{L(1), L(2), L(-3)} {} };
struct F {
  M m;
  F() try : m(-5) {} catch (int) { note(55); }
};

struct AG { L x; int k; L y; };
struct DF { DF() { note(7); } ~DF() { note(-7); } DF(int v) { may_throw(v); note(8); } };

bool same(const int* want, int count) {
  if (n != count) return false;
  for (int i = 0; i < count; ++i)
    if (log_[i] != want[i]) return false;
  return true;
}

int main() {
  n = 0;
  if (locals() != 0) return 1;
  { const int w[] = {1, 2, -102, -101}; if (!same(w, 4)) return 2; }

  n = 0;
  try { C c(-2); return 3; } catch (int) {}
  { const int w[] = {10, 11, -11, -10}; if (!same(w, 4)) return 4; }

  n = 0;
  try { D d(-1); return 5; } catch (int) {}
  { const int w[] = {11, -11}; if (!same(w, 2)) return 6; }

  n = 0;
  try { E e; return 7; } catch (int x) { if (x != -3) return 8; }
  { const int w[] = {1, 2, -102, -101}; if (!same(w, 4)) return 9; }

  n = 0;
  try { F f; return 10; } catch (int x) { if (x != -5) return 11; }
  { const int w[] = {55}; if (!same(w, 1)) return 12; }

  n = 0;
  try { AG g = {L(1), may_throw(2), L(-4)}; return 13; } catch (int x) { if (x != -4) return 14; }
  { const int w[] = {1, -101}; if (!same(w, 2)) return 15; }

  n = 0;
  try { AG g = {L(1), may_throw(-6), L(2)}; return 16; } catch (int x) { if (x != -6) return 17; }
  { const int w[] = {1, -101}; if (!same(w, 2)) return 18; }

  n = 0;
  try { L a[2][2] = {{L(1), L(2)}, {L(-3), L(4)}}; return 19; } catch (int) {}
  { const int w[] = {1, 2, -102, -101}; if (!same(w, 4)) return 20; }

  n = 0;
  try { DF a[3] = {DF(1)}; } catch (int) { return 21; }
  { const int w[] = {8, 7, 7, -7, -7, -7}; if (!same(w, 6)) return 22; }
  return 0;
}
