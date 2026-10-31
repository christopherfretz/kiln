// [temp.expl.spec], [temp.explicit]: explicit specializations and
// instantiations are called at run time.
//@ expect: run
//@ stdout: 1 2 3 4 5
extern "C" int printf(const char*, ...);
template <class T> int f(T) { return 1; }
template <> int f(int) { return 2; }
template <class T> struct A { int g() { return 3; } };
template <> int A<char>::g() { return 4; }
template <class T> int k(T t) { return t + 5; }
template int k(int);
int main() {
  printf("%d %d %d %d %d\n", f(1.0), f(1), A<int>{}.g(), A<char>{}.g(), k(0));
}
