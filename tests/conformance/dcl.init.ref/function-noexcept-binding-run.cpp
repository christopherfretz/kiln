// [dcl.init.ref]/5.1, [conv.fctptr]: a reference to function binds directly to a
// noexcept function and calls through it; overload ranking treats the binding as identity.
//@ expect: run
int v;
void f(int x) noexcept { v += x; }
void (&r1)(int) = f;
void (&&r2)(int) = f;
int pick(void (&)(int)) { return 1; }
int pick(...) { return 2; }
long via(void (*)(int)) { return 1; }
long via(void (*)(int) noexcept) { return 2; }
int main() {
  r1(1);
  r2(2);
  void (&r3)(int) = f;
  r3(4);
  if (v != 7) return 1;
  if (pick(f) != 1) return 2;
  if (via(f) != 2) return 3;
  return 0;
}
