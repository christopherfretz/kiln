// [temp.pre]/8, [temp.friend]/5, [class.friend]: a local class of a
// function template (or of a generic lambda in one) is templated, so a
// member friend whose qualifier depends on the enclosing template
// arguments (`friend int B<T, int>::f();`) is matched when the local class
// is instantiated, not at its definition; it then grants access.
//@ expect: run
//@ stdout: 2 2 4 6
#include <cstdio>
template <class T, class U> struct B { static int f(); static int h(); };
template <class T> int g() {
  struct L {
    friend int B<T, int>::f();
  };
  return B<T, int>::f();
}
template <class T> int k() {
  auto l = [](auto u) {
    struct L {
      friend int B<T, decltype(u)>::h();
    private:
      static int p() { return 4; }
    };
    return B<T, decltype(u)>::h();
  };
  return l(1);
}
template <class T, class U> int B<T, U>::f() { return 2; }
template <class T, class U> int B<T, U>::h() { return 4 + sizeof(T); }
int main() { std::printf("%d %d %d %d\n", g<char>(), B<int, int>::f(), k<int>() - 4, k<short>()); }
