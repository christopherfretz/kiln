// [expr.prim.lambda.closure]/2: the closure type is declared in the
// smallest enclosing scope; classes local to the lambda body are local to
// its call operator.
//@ expect: run
//@ stdout: 5 7
extern "C" int printf(const char*, ...);
int main() {
  auto mk = [](int v) {
    struct Box { int v; int get() const { return v; } };
    return Box{v}.get();
  };
  auto mk2 = [](int v) {
    struct Base { virtual int f() const { return 1; } };
    struct D : Base { int v; D(int v) : v(v) {} int f() const override { return v; } };
    D d(v);
    const Base& b = d;
    return b.f();
  };
  printf("%d %d\n", mk(5), mk2(7));
}
