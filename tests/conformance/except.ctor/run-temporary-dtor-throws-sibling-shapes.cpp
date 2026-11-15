// [except.ctor]/2-4: a temporary's destructor that throws at the end of a
// full-expression destroys every object completed in it: a delegating
// constructor's object, the result of `?:`, a lambda's captures, a member
// initialized from a constructor argument, a variable initialized by a
// comma expression. A second throw while unwinding calls terminate.
// Found running GCC's C++ testsuite. clang 19 leaks all but the last
// two; GCC 16 leaks the delegating constructor's object.
//@ expect: run
//@ oracle: none
//@ xfail(clang): clang 19 and 22 leak the completed objects
//@ stdout: deleg 0 cond 0 lambda 0 arg 0 comma 0 terminate 0
extern "C" int printf(const char*, ...);
extern "C" void exit(int);
namespace std { using terminate_handler = void (*)(); terminate_handler set_terminate(terminate_handler) noexcept; }
int live;
struct A {
  int i;
  A(int i = 0) : i(i) { ++live; }
  A(const A& o) : i(o.i) { ++live; }
  ~A() { --live; }
};
struct Q { ~Q() noexcept(false) { throw 1; } };
struct T { int i; T(int i) : i(i) {} ~T() noexcept(false) { if (i == 2) throw 2; } };
struct Agg { A a; A b; };
struct D {
  A a;
  D(int) : a(1) {}
  D() : D((Q(), 1)) {}
};
struct E { A a; E(A x, const Q& = Q()) : a(x) {} };
int main() {
  try { D d; } catch (int) {}
  printf("deleg %d ", live); live = 0;
  try {
    bool c = live == 0;
    Agg g = c ? Agg{A(1), (Q(), A(2))} : Agg{A(3), A(4)};
  } catch (int) {}
  printf("cond %d ", live); live = 0;
  try {
    A x(1);
    auto l = [x, y = (Q(), A(2))] { return x.i + y.i; };
    (void)l;
  } catch (int) {}
  printf("lambda %d ", live); live = 0;
  try { E e(A(1)); } catch (int) {}
  printf("arg %d ", live); live = 0;
  try { A a = (T(2), A(1)); } catch (int) {}
  printf("comma %d ", live); live = 0;
  std::set_terminate([] { printf("terminate %d\n", live); exit(0); });
  try { T t(2); A a(1); throw 1; } catch (int) {}
  printf("no terminate\n");
}
