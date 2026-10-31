//@ expect: ast
// [except.pre]/1, [except.handle]: try-blocks, handlers by value, by
// reference, by pointer and catch-all; a class handler variable is
// copy-initialized from the exception object ([except.handle]/15);
// function-try-blocks of a function and a constructor.
struct E { E(); E(const E &); ~E(); };
struct T { int x; };
void g();
void f() {
  try {
    g();
  } catch (E e) {
  } catch (T t) {
  } catch (int &) {
    throw;
  } catch (const E *p) {
    throw p;
  } catch (...) {
  }
}
int h(int a) {
  try {
    return 1;
  } catch (int x) {
    E e;
    throw e;
  }
}
void k(int a) try {
  g();
} catch (int) {
  throw a;
}
struct C {
  int m;
  C(int v) try : m(v) {
    g();
  } catch (...) {
  }
};
void nested() {
  try {
    try {
      g();
    } catch (int) {
    }
  } catch (E &e) {
    throw;
  }
}
