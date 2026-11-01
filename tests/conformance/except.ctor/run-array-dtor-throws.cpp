// [except.ctor]/2: if the destructor of an array element throws, the
// elements not yet destroyed are still destroyed, in reverse order, before
// the handler runs; the same for an array member destroyed by its class's
// destructor.
//@ expect: run
//@ stdout: d2 throw d1 d0 caught | d1 throw d0 caught
extern "C" int printf(const char*, ...);
struct S {
  int i;
  static int boom;
  static int next;
  S() : i(next++) {}
  ~S() noexcept(false) {
    printf("d%d ", i);
    if (i == boom) { printf("throw "); throw 1; }
  }
};
int S::boom = 2;
int S::next = 0;
struct G { S m[2]; ~G() noexcept(false) {} };
int main() {
  try { S a[3]; } catch (int) { printf("caught | "); }
  S::next = 0;
  S::boom = 1;
  try { G g; } catch (int) { printf("caught\n"); }
}
