// [basic.def.odr]/10, [basic.def.odr]/5: naming a local constant in a local
// class's or enumeration's scope is valid when it is no odr-use (the
// lvalue-to-rvalue conversion of a variable usable in constant expressions,
// an unevaluated operand), inside a lambda body or not; the lambda then
// captures nothing for it. clang 22 and GCC 16.1 agree.
//@ expect: run
//@ strict-diags
int f() {
  const int i = 2;
  return [=] {
    enum E { a = i, s = sizeof(*&i) };
    struct L {
      int b : i;
      int arr[i + 1];
      int g() noexcept(i == 2) { return i; }
    };
    L l{};
    l.b = 1;
    return a + int(s) + int(sizeof l.arr / sizeof(int)) + l.g() + l.b;
  }();
}
int main() {
  const int n = 3;
  enum G { d = n };
  struct M { int w : n; };
  return f() == 2 + int(sizeof(int)) + 3 + 2 + 1 && d == 3 && sizeof(M) == sizeof(int) ? 0 : 1;
}
