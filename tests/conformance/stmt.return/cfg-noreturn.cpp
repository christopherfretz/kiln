// [dcl.attr.noreturn]/1, [except.throw]/1, [except.handle]/1,
// [class.dtor]/14, [class.base.init]/1: calls of noreturn functions end
// their block in the exit; try blocks dispatch to their handlers; scope
// ends run destructors; constructors initialize their members (the
// control-flow graph, spec/sema.md §7.5.4).
//@ expect: cfg
[[noreturn]] void ab();
void g();

void noret(int x) {
  if (x)
    g();
  else
    ab();
  g();
}

void thrower(int x) {
  if (x)
    throw 1;
  g();
}

void handler() {
  try {
    g();
  } catch (int) {
    g();
  } catch (...) {
  }
}

struct D {
  ~D();
};

void dtors(int x) {
  D d;
  if (x)
    return;
  g();
}

struct C {
  int a;
  int b;
  C(int x) : a(x), b(2) {}
};
