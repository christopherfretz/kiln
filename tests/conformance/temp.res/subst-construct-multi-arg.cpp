// [temp.res.general]/1, [temp.inst]/3, [dcl.init.general]/16.6.2, /9:
// constructions with no or several arguments, of a class named directly
// and through a template parameter, substituted twice (a member template
// of a class template, then a generic lambda in it). Each is rebuilt as the
// direct-initialization (or, with no argument and `()`, value-initialization)
// of the substituted class from the substituted arguments, and calls the
// same constructor as written outside a template.
//@ expect: run
//@ stdout: 2 0 3 2 0 0 4 0 3 0 2 0 | 0 0 0
extern "C" int printf(const char*, ...);

struct S {
  int n;
  S() : n(0) {}
  S(int, int) : n(2) {}
  S(int, int, int) : n(3) {}
};
struct V { int z; };

template <class I, class T> struct O {
  template <class U> static void f(U u) {
    I t{};
    printf("%d %d %d ", S(t, 1).n, S().n, T(1, 2, u).n);
    S a(t, 1);
    T b;
    T* p = new T(1, 2, 3);
    T* q = new T[2]();
    printf("%d %d %d %d %d ", a.n, b.n, q[1].n, (int)sizeof(S(t, 1)), (int)sizeof(T(t, 1, 2)) - 4);
    auto l = [&](auto v) {
      T x(1, v);
      return x.n + T(v, 1).n * 0 + T().n;
    };
    printf("%d %d %d %d ", p->n, l(1) - 2, l(2L), T().n);
    delete p;
    delete[] q;
  }
};

template <class T> T zeroed() { return T(); }

int main() {
  O<int, S>::f(1);
  printf("| %d %d %d\n", zeroed<V>().z, zeroed<S>().n, S(zeroed<S>().n, 1).n - 2);
}
