// Mangling differential: local entities, static locals and their guards,
// reference temporaries, unnamed classes, explicit instantiations.
int seed();
namespace loc {
struct K {
  K();
  int v;
};
int one() {
  static int a = seed();
  static K k;
  struct L {
    int g() { return 1; }
    static int h() { return 2; }
  };
  L l;
  return a + l.g() + L::h() + k.v;
}
int two(int x) {
  if (x) {
    static int s = seed();
    return s;
  }
  static int s = seed() + 1;
  return s;
}
struct M {
  int method(int) const;
  M();
  ~M();
};
int M::method(int) const {
  static int q = seed();
  struct Inner {
    int z() const { return 3; }
  };
  return q + Inner().z();
}
M::M() {
  static int in_ctor = seed();
}
M::~M() {
  static int in_dtor = seed();
}
const int &ref = seed();
int &&rref = seed() + 1;
static const int &sref = seed();
const int &use_sref() { return sref; }
const int &local_ref() {
  static const int &lr = seed();
  return lr;
}
typedef struct {
  int m;
  void f() {}
} Named;
void use_named() { Named().f(); }
} // namespace loc

template <class T> struct Inst {
  T t;
  void f() {}
  int g(T) { return 0; }
  static T st;
  struct Nest {
    void h() {}
  };
};
template <class T> T Inst<T>::st;
template struct Inst<int>;
template struct Inst<loc::K>;
template struct Inst<Inst<char>>;
extern "C" int cfun() {
  static int cs = seed();
  return cs;
}
int main() {
  static int ms = seed();
  return ms;
}
