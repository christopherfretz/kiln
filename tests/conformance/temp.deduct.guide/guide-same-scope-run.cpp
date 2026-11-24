// [temp.deduct.guide]/3, [over.match.class.deduct]/1: a deduction guide in
// the scope its class template belongs to is used by class template
// argument deduction: in a reopened namespace, through a linkage
// specification, in a reopened inline namespace, and a member guide with
// its template's access (used inside the class). Reduced from real code.
//@ expect: run
//@ std: c++20
//@ stdout: 8 4 2 8 8
#include <cstdio>
namespace n { template<class T> struct A { T v; A(T t) : v(t) {} }; }
namespace n { A(int) -> A<long long>; }
namespace n { extern "C++" { A(short) -> A<int>; } }
namespace o {
inline namespace v { template<class T> struct B { T t{}; template<class X> B(X) {} }; }
}
namespace o { inline namespace v { template<class T> B(T *) -> B<char[2]>; } }
using namespace n;
using namespace o;
class P {
  template<class T> struct M { T t{}; template<class X> M(X) {} };
  template<class T> M(T *) -> M<double>;
public:
  static unsigned long size() { M m((int *)nullptr); return sizeof(m.t); }
};
template<class U> struct Q {
  template<class T> struct M { T t; M(T x) : t(x) {} };
  M(int) -> M<U>;
  static unsigned long size() { M m(1); return sizeof(m.t); }
};
int main() {
  A a(1);
  A b((short)1);
  int i = 0;
  B c(&i);
  std::printf("%zu %zu %zu %lu %lu\n", sizeof(a.v), sizeof(b.v), sizeof(c.t),
              P::size(), Q<double>::size());
}
