// [stmt.dcl]/3: a block-scope static with dynamic initialization is
// initialized the first time control passes through its declaration; if
// the initialization exits by an exception it is retried on the next
// entry; it is destroyed at exit ([basic.start.term]/1). libc++
// support/count_new.h's getGlobalMemCounter.
//@ expect: run
//@ stdout: start C1 1 1 2 throw C2 3 done 3 ~C2 ~C1
extern "C" int printf(const char*, ...);

struct C {
  int v;
  C(int x) : v(x) { printf("C%d ", v); }
  ~C() { printf("~C%d ", v); }
};
inline C *get() {
  static C c(1);
  return &c;
}
int calls = 0;
int next() {
  if (++calls == 1) throw 0;
  return calls;
}
int retry() {
  static C r(next());
  return r.v + 1;
}
int counter() {
  static int n = 0;
  return ++n;
}

int main() {
  printf("start ");
  printf("%d %d ", get()->v, get() == get());
  printf("%d ", counter() + counter() - 1);
  try {
    retry();
  } catch (int) {
    printf("throw ");
  }
  printf("%d ", retry());
  printf("done %d ", counter());
  return 0;
}
