// [temp.expl.spec]/1: an explicit specialization of a static data member
// of an implicitly instantiated class template specialization replaces
// the pattern's definition; other specializations still instantiate it.
//@ expect: run
//@ stdout: 9 1 1 7
extern "C" int printf(const char *, ...);

template <class T> struct A {
  static int s;
  static T t;
  void g();
};
template <class T> int A<T>::s = 1;
template <class T> T A<T>::t = T(1);
template <> int A<long>::s = 9;
template <> char A<char>::t = 7;

int main() {
  printf("%d %d %d %d\n", A<long>::s, A<int>::s, A<char>::s, A<char>::t);
}
