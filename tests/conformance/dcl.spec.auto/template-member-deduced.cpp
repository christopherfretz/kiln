// [dcl.spec.auto.general]: a member function of a class template with a
// deduced return type is deduced at instantiation, called through '.'.
//@ expect: run
//@ stdout: 2 3 7
extern "C" int printf(const char*, ...);
template <class T> struct S {
  T v;
  auto g() { return v + 1; }
  decltype(auto) ref() { return (v); }
};
template <class T> auto twice(T t) { return t + t; }
int main() {
  S<int> s{1};
  s.ref() = 2;
  printf("%d %d %d\n", s.v, s.g(), twice(3) + 1);
  return 0;
}
