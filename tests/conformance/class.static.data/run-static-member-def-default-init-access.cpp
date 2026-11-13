// [class.static.data]/3, [class.access]/7: the initializer of the definition
// of a static data member is in the scope of its class, so the names it
// uses, and the constructor a default-initialization calls
// ([dcl.init.general]/7), are access checked as from a member: a private
// default constructor initializes `inst`. Without an initializer as with
// one, for a class, an explicit specialization (a downstream Metrics
// a metrics macro), a member of a partial
// specialization and of a class template (both instantiated), and a nested
// class.
//@ expect: run
//@ exit: 63
int n;
template <typename T> class F;
template <> class F<int> {
  static F<int> inst;
  F() { n += 1; }
};
F<int> F<int>::inst;

class G { static G inst; G() { n += 2; } };
G G::inst;

template <typename T> class P;
template <typename T> class P<T *> {
  static P inst;
  P() { n += 4; }
public:
  static void *touch() { return &inst; }
};
template <typename T> P<T *> P<T *>::inst;

template <typename T> class Q {
  static Q inst;
  Q() { n += 8; }
public:
  static void *touch() { return &inst; }
};
template <typename T> Q<T> Q<T>::inst;

class O {
  class I { friend class O; I() { n += 16; } };
  static I inst;
};
O::I O::inst;

class H { static H inst; H() { n += 32; } };
H H::inst{};

int main() {
  P<int *>::touch();
  Q<int>::touch();
  return n;
}
