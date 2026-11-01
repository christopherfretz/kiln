// [temp.expl.spec]/1, /7, [temp.inst]/3: `template<> struct X<int>::N`,
// `template<> enum class X<int>::F : int`, and a member class template
// specialized for one implicit specialization of its enclosing class. A
// declaration-only member specialization is defined later; a scoped member
// enumeration is defined only when required (a nested-name-specifier,
// `using enum`, an enumerator named in an instantiated member function), so
// it may be specialized after its class is instantiated; other
// specializations are still instantiated from the pattern
//@ expect: run
//@ strict-diags
extern "C" int printf(const char *, ...);
template <class T>
struct X {
  struct N {
    T a = 1;
    int f() const { return 10; }
  };
  struct D;
  enum class F : int { a = 1, b };
  enum class G : int;
  template <class U>
  struct M {
    int g() const { return 100; }
  };
  struct Outer {
    struct Inner {
      int h() const { return 7; }
    };
  };
  int fb() const { return int(F::b); }
};
template <class T>
struct X<T>::D {
  int d() const { return 3; }
};
template <class T>
enum class X<T>::G : int { g = 4 };

X<int> xi;  // the class is instantiated; F, G and D are not yet defined

template <>
struct X<int>::N {
  long z = 7;
  int f() const;
};
int X<int>::N::f() const { return 42; }
template <>
struct X<int>::D;
template <>
enum class X<int>::F : int { p = 10, q = 20 };
template <>
enum class X<int>::G : int { g = 50 };
template <>
template <>
struct X<int>::M<char> {
  int g() const { return 200; }
};
template <>
struct X<long>::Outer::Inner {
  int h() const { return 70; }
};
template <>
struct X<int>::D {
  int d() const { return 30; }
};

int use_enum() {
  using enum X<int>::F;
  return int(p) + int(q);
}
int use_pattern_enum() {
  using enum X<short>::F;
  return int(a) + int(b);
}

int main() {
  X<int>::N n;
  X<long>::N m;
  X<int>::D d;
  X<long>::D dl;
  X<int>::M<char> mc;
  X<int>::M<int> mi;
  X<long>::Outer::Inner in;
  X<int>::Outer::Inner ii;
  X<long> xl;
  printf("%d %ld %d %ld\n", n.f(), n.z, m.f(), m.a);
  printf("%d %d %d %d\n", d.d(), dl.d(), mc.g(), mi.g());
  printf("%d %d %d\n", in.h(), ii.h(), xl.fb());
  printf("%d %d %d %d\n", int(X<int>::F::q), int(X<int>::G::g), int(X<long>::G::g), int(X<long>::F::b));
  printf("%d %d %zu\n", use_enum(), use_pattern_enum(), sizeof(X<int>::F));
  return 0;
}
