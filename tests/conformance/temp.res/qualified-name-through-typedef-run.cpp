// [temp.res.general]/1, [temp.inst]/2: names found through a typedef of a
// not yet instantiated member class of a class template specialization
// resolve to the instantiated members at run time (WP-288; see
// qualified-name-through-typedef.cpp).
//@ expect: run
//@ stdout: 7 3 1
#include <cstdio>

template <class T>
struct L {
  struct P {
    typedef L distribution_type;
    typedef T I;
    static const int v = 7;
    static int g() { return 3; }
  };
  int id() const { return 1; }
};

int main() {
  typedef L<int>::P Q;
  typedef Q::I I;
  typedef Q::distribution_type D;
  I a = Q::v;
  D d;
  std::printf("%d %d %d\n", a, Q::g(), d.id());
}
