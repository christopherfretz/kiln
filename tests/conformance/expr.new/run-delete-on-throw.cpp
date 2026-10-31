// [expr.new]/28: if the initialization of a new-expression throws, the
// matching deallocation function is called (class-specific usual and
// placement forms), and not when it completes.
//@ expect: run
//@ strict-diags
using size_t = decltype(sizeof 0);
int deletes = 0;
int placement_deletes = 0;
alignas(16) unsigned char pool[256];
int may_throw(int v) { if (v < 0) throw v; return v; }

struct S {
  int v;
  S(int x) : v(may_throw(x)) {}
  static void* operator new(size_t) { return pool; }
  static void operator delete(void*, size_t) { ++deletes; }
  static void* operator new(size_t, int) { return pool + 64; }
  static void operator delete(void*, int) { ++placement_deletes; }
};
struct K { K() { may_throw(count++ == 2 ? -1 : 1); } static int count; };
int K::count = 0;

int main() {
  S* p = new S(1);
  if (p->v != 1 || deletes != 0) return 1;
  try { new S(-1); return 2; } catch (int) {}
  if (deletes != 1) return 3;
  try { new (5) S(-2); return 4; } catch (int) {}
  if (placement_deletes != 1 || deletes != 1) return 5;
  S* q = new (5) S(3);
  if (q != (void*)(pool + 64) || placement_deletes != 1) return 6;
  try { K* k = new K[4]; (void)k; return 7; } catch (int) {}
  if (K::count != 3) return 8;
  return 0;
}
