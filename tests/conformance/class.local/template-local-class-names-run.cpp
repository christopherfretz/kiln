// [class.local], [temp.inst]/3 (CWG1484), [temp.dep.type]: a local class of
// a function template, of a lambda in one, and of a generic lambda in one,
// is instantiated with the enclosing function; its member bodies name the
// enclosing function's local enumeration and enumerators, typedef and
// alias, static local, other local class and using-declaration, and the
// members of an enclosing local class. Each reference is checked in the
// output, so a statement dropped at instantiation shows (WP-905).
//@ expect: run
//@ stdout: 2 3 4 5 6 7 8 9 | 2 3 4 5 6 7 8 9 | 2 3 4 5 6 7 8 9
#include <cstdio>

namespace ns { int nv = 8; }

template <class T> int direct() {
  enum E { A = 2, B };
  typedef T Int;
  using Alias = T;
  static int s = 4;
  struct Other { int v() { return 5; } };
  using ns::nv;
  struct O {
    int ov = 6;
    static int so() { return 7; }
    struct I {
      int run(O &o) {
        Int a = A; Alias b = B;
        std::printf("%d %d %d %d ", a, b, s, Other().v());
        std::printf("%d %d %d %d", o.ov, so(), nv + 0 * E(), 9);
        return a + b;
      }
    };
  };
  O o;
  typename O::I i;
  return i.run(o);
}

template <class T> int in_lambda() {
  auto l = [] {
    enum E { A = 2, B };
    typedef T Int;
    using Alias = T;
    static int s = 4;
    struct Other { int v() { return 5; } };
    using ns::nv;
    struct O {
      int ov = 6;
      static int so() { return 7; }
      struct I {
        int run(O &o) {
          Int a = A; Alias b = B;
          std::printf("%d %d %d %d ", a, b, s, Other().v());
          std::printf("%d %d %d %d", o.ov, so(), nv + 0 * E(), 9);
          return a + b;
        }
      };
    };
    O o;
    typename O::I i;
    return i.run(o);
  };
  return l();
}

template <class T> int in_generic_lambda() {
  auto l = [](auto z) {
    enum E { A = 2, B };
    typedef decltype(z) Int;
    using Alias = T;
    static int s = 4;
    struct Other { int v() { return 5; } };
    using ns::nv;
    struct O {
      int ov = 6;
      static int so() { return 7; }
      struct I {
        int run(O &o) {
          Int a = A; Alias b = B;
          std::printf("%d %d %d %d ", a, b, s, Other().v());
          std::printf("%d %d %d %d", o.ov, so(), nv + 0 * E(), 9);
          return a + b;
        }
      };
    };
    O o;
    typename O::I i;
    return i.run(o);
  };
  return l(0);
}

int main() {
  int r = direct<int>();
  std::printf(" | ");
  r += in_lambda<int>();
  std::printf(" | ");
  r += in_generic_lambda<int>();
  std::printf("\n");
  return r == 15 ? 0 : 1;
}
