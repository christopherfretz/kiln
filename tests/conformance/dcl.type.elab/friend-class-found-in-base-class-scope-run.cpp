// [dcl.type.elab]/4, [class.friend]/2: the unqualified lookup of
// `friend class X;` stops at the innermost enclosing namespace but does
// search the class scope, base classes included ([class.member.lookup]).
// A class found there, even one declared in another namespace, is the
// friend: here the injected-class-name of a private base specialization
// (LLVM's TrailingObjects users, `friend class TrailingObjects;`) and a
// member class of a base. No new class is declared in the current
// namespace.
//@ expect: run
//@ stdout: 7 9 1
#include <cstdio>

namespace lib {
template <class Derived, class T> class Trailing {
protected:
  static T *at(Trailing *p) { return reinterpret_cast<T *>(p); }

public:
  // Derived-to-private-base conversion: needs friendship.
  T *get() { return at(static_cast<Derived *>(this)); }
};

struct Base {
  struct Peer;
};
} // namespace lib

namespace app {
class E final : private lib::Trailing<E, int> {
  friend class Trailing;
  int v = 7;

public:
  int first() { return *get(); }
};

struct C : lib::Base {
  friend class Peer;

private:
  int secret = 9;
};
} // namespace app

struct lib::Base::Peer {
  static int read(app::C &c) { return c.secret; }
};

namespace app {
// No app::Peer was declared by the friend declaration.
struct Peer {
  static constexpr int fresh = 1;
};
} // namespace app

int main() {
  app::E e;
  app::C c;
  std::printf("%d %d %d\n", e.first(), lib::Base::Peer::read(c), app::Peer::fresh);
}
