// [class.temporary]/6, [except.ctor]/1-4: a temporary bound to a
// reference is destroyed during stack unwinding, also when it was created
// in a conditional branch or inside a braced aggregate; a conditional
// temporary at a computed address is destroyed when its full-expression
// throws; a delegating constructor of a class with virtual bases destroys
// the object (base-object variant with the VTT) when its body throws; and
// a conditional new-expression with class placement arguments calls the
// matching deallocation function when the constructor throws.
//@ expect: run
//@ strict-diags
typedef decltype(sizeof 0) size_t;
int log_[32];
int n = 0;
void note(int v) { log_[n++] = v; }
int may_throw(int v) { if (v < 0) throw v; return v; }
bool same(const int* w, int k) {
  if (n != k) return false;
  for (int i = 0; i < k; ++i) if (log_[i] != w[i]) return false;
  return true;
}

struct T { int id; T(int i) : id(i) { note(i); } ~T() { note(-id); } int v() const { return id; } };
struct A { const T& t; int x; };
struct AT { T a[2]; int x; };

int bound(int c) {
  try {
    const T& r = c ? T(1) : T(2);
    may_throw(-r.v());
  } catch (int) { return 0; }
  return 1;
}

int in_aggregate(int c) {
  try {
    const A& a = c ? A{T(3), may_throw(c)} : A{T(4), may_throw(c - 1)};
    may_throw(-a.t.v());
  } catch (int) { return 0; }
  return 1;
}

int computed_address(int c) {
  try {
    may_throw(c ? AT{{T(5), T(6)}, may_throw(-7)}.x : 0);
  } catch (int) { return 0; }
  return 1;
}

struct V { V() { note(20); } ~V() { note(-20); } };
struct B : virtual V {
  B(int) { note(21); }
  B() : B(0) { may_throw(-22); }
  ~B() { note(-21); }
};
struct D : B { D() : B() {} };

struct Tag { int k; };
void* operator new(size_t, Tag t, void* p) { note(30 + t.k); return p; }
void operator delete(void*, Tag t, void*) { note(-30 - t.k); }
struct C { C(int v) { may_throw(v); } };
alignas(C) unsigned char buf[sizeof(C)];

int main() {
  n = 0;
  if (bound(1) != 0) return 1;
  { const int w[] = {1, -1}; if (!same(w, 2)) return 2; }
  n = 0;
  if (bound(0) != 0) return 3;
  { const int w[] = {2, -2}; if (!same(w, 2)) return 4; }

  n = 0;
  if (in_aggregate(1) != 0) return 5;
  { const int w[] = {3, -3}; if (!same(w, 2)) return 6; }
  n = 0;
  if (in_aggregate(-1) != 0) return 7;
  { const int w[] = {3, -3}; if (!same(w, 2)) return 8; }

  n = 0;
  if (computed_address(1) != 0) return 9;
  { const int w[] = {5, 6, -6, -5}; if (!same(w, 4)) return 10; }

  n = 0;
  try { D d; return 11; } catch (int x) { if (x != -22) return 12; }
  { const int w[] = {20, 21, -21, -20}; if (!same(w, 4)) return 13; }

  n = 0;
  try { C* p = n == 0 ? new (Tag{1}, buf) C(-1) : nullptr; (void)p; return 14; } catch (int) {}
  { const int w[] = {31, -31}; if (!same(w, 2)) return 15; }
  return 0;
}
