// [dcl.fct.def.general]/8 (and the GNU `__FUNCTION__` and
// `__PRETTY_FUNCTION__`): the predefined identifiers name the enclosing
// function as clang 19 spells them: the template argument suffix of
// function and class template specializations (outermost first, function
// template arguments printed with their types), the instantiation pattern's
// parameter types, lambdas (`auto`), constructors and destructors (no return
// type), operators, conversion functions, deduced and trailing return types,
// local classes, and "top level" outside a function.
//@ expect: run
//@ args: -Wno-predefined-identifier-outside-function
//@ stdout: void h() [N = 1, U = 2U, C = 'a', B = true, L = 5L, Ts = <int, char *>]
//@ stdout: void A<int>::f(T) [T = int, N = 3]
//@ stdout: static int A<int>::g(U, T) [T = int, N = 3, U = double]|g
//@ stdout: int A<int>::operator int() const & [T = int, N = 3]|operator int
//@ stdout: virtual int ns::B::v(int, ...) volatile
//@ stdout: int ns::tr(int)
//@ stdout: void A<char>::f()
//@ stdout: void k(T) [T = double]
//@ stdout: void k(int)
//@ stdout: top level||top level
//@ stdout: auto main()::(anonymous class)::operator()(auto) const [x:auto = int]
//@ stdout: void main()::Loc::m()
//@ stdout: const TypeID &makeTypeIDImp() [T = int]
//@ stdout: S S S::S()
//@ stdout: int S::operator+(int)
//@ stdout: auto g(T) [T = double]
//@ stdout: auto g(double)::(anonymous class)::operator()(int) const|operator()
//@ stdout: auto g(T) [T = char]
//@ stdout: auto g(char)::(anonymous class)::operator()(int) const|operator()
//@ stdout: int main()
//@ stdout: S::~S()
//@ stdout: virtual A<int>::~A() [T = int, N = 3]
extern "C" int printf(const char*, ...);

template <int N, unsigned U, char C, bool B, long L, class... Ts>
void h() { printf("%s\n", __PRETTY_FUNCTION__); }

template <class T, int N = 3>
struct A {
  void f(T) { printf("%s\n", __PRETTY_FUNCTION__); }
  template <class U>
  static int g(U, T) {
    printf("%s|%s\n", __PRETTY_FUNCTION__, __FUNCTION__);
    return 0;
  }
  virtual ~A() { printf("%s\n", __PRETTY_FUNCTION__); }
  operator int() const & {
    printf("%s|%s\n", __PRETTY_FUNCTION__, __func__);
    return 0;
  }
};

namespace ns {
struct B {
  virtual int v(int, ...) volatile {
    printf("%s\n", __PRETTY_FUNCTION__);
    return 0;
  }
};
auto tr(int x) -> decltype(x) {
  printf("%s\n", __PRETTY_FUNCTION__);
  return x;
}
}  // namespace ns

template <>
struct A<char> {
  void f() { printf("%s\n", __PRETTY_FUNCTION__); }
};

template <class T> void k(T) { printf("%s\n", __PRETTY_FUNCTION__); }
template <> void k<int>(int) { printf("%s\n", __PRETTY_FUNCTION__); }

const char* top = __PRETTY_FUNCTION__;
const char* top2 = __func__;
struct D { const char* m = __PRETTY_FUNCTION__; };

struct TypeID {
  const char* s;
  constexpr TypeID(const char* p) : s(p) {}
};
template <class T>
const TypeID& makeTypeIDImp() {
  static const TypeID id(__PRETTY_FUNCTION__);
  return id;
}

struct S {
  S() { printf("%s %s %s\n", __func__, __FUNCTION__, __PRETTY_FUNCTION__); }
  ~S() { printf("%s\n", __PRETTY_FUNCTION__); }
  int operator+(int) {
    printf("%s\n", __PRETTY_FUNCTION__);
    return 0;
  }
};

template <class T>
auto g(T t) {
  printf("%s\n", __PRETTY_FUNCTION__);
  auto l = [](int x) {
    printf("%s|%s\n", __PRETTY_FUNCTION__, __func__);
    return x;
  };
  l(1);
  return t;
}

int main() {
  h<1, 2, 'a', true, 5L, int, char*>();
  A<int> a;
  a.f(1);
  A<int>::g(2.0, 1);
  (void)(int)a;
  ns::B b;
  b.v(1);
  ns::tr(1);
  A<char>().f();
  k(1.0);
  k(1);
  printf("%s|%s|%s\n", top, top2, D().m);
  auto l = [](auto x) { printf("%s\n", __PRETTY_FUNCTION__); };
  l(1);
  struct Loc {
    void m() { printf("%s\n", __PRETTY_FUNCTION__); }
  };
  Loc().m();
  printf("%s\n", makeTypeIDImp<int>().s);
  {
    S s;
    s + 1;
    g(1.0);
    g<char>('a');
    printf("%s\n", __PRETTY_FUNCTION__);
  }
}
