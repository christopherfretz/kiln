// [temp.dep.type]/6, [basic.lookup.qual]/1, [namespace.udecl]/1: in
// `class B<T>::In::Deep` inside another class template, `In` names a member
// of an unknown specialization. When `B<A>` is an explicit specialization
// whose `In` is an alias, a typedef or a using-declaration of a base's
// class, the name is looked up again in `B<A>` and `Deep` found in the
// class it denotes (kiln reported "not yet implemented"; WP-1071). The
// `typename` and call forms take the same lookup. g++ 14 prints the same.
//@ expect: run
//@ skip(clang): clang 19 and 22 crash on a member class of a specialization named through `class B<T>::In::`
//@ stdout: 1 7 7 9
//@ stdout: 1 8 8 10
//@ stdout: 1 8 8 10
#include <cstdio>
struct Other { static int f(int) { return 7; } struct Deep { int y = 8; }; };
struct Base { struct In { static int f(int) { return 9; } struct Deep { int y = 10; }; }; };
template <class T> struct B { struct In { static int f(int) { return 1; } struct Deep { int y = 1; }; }; };
template <> struct B<long> { using In = Other; };
template <> struct B<short> { typedef Other In; };
template <> struct B<char> : Base { using Base::In; };
template <class T> struct W {
  static int g() { return B<T>::In::f(1); }
  typename B<T>::In::Deep d;
};
template <class T> struct V { class B<T>::In::Deep d; };
int main() {
  std::printf("%d %d %d %d\n", W<int>::g(), W<long>::g(), W<short>::g(), W<char>::g());
  std::printf("%d %d %d %d\n", W<int>().d.y, W<long>().d.y, W<short>().d.y, W<char>().d.y);
  std::printf("%d %d %d %d\n", V<int>().d.y, V<long>().d.y, V<short>().d.y, V<char>().d.y);
}
