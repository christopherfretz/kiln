// [dcl.attr.annotation]/1, /3, [temp.expl.spec], [temp.explicit],
// [meta.reflection.annotation] (P3394R4): annotations are per declaration.
// An explicit specialization of a function template (namespace scope,
// static or non-static member template, a member template of a class
// template, `template<> template<>`) or of a member function of a class
// template is its own entity: annotations_of reports only what its own
// declarations say, first to latest, never the pattern's. An implicit or
// explicit instantiation carries the pattern's (an explicit instantiation
// adds its own after them); a parameter keeps the pattern parameter's.
// Variable and class templates behave the same way. GCC 16.1 compiles and
// runs the same file; the output must agree (WP-1216).
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ stdout: f<int>: 8
//@ stdout: f<char>:
//@ stdout: f<short>: 9 10 11
//@ stdout: f<long>: 1
//@ stdout: f<unsigned>: 1 14
//@ stdout: f<double>: 1
//@ stdout: dd<int>:
//@ stdout: dd<char>: 20
//@ stdout: dd<short>: 2 15
//@ stdout: dd<long>: 8
//@ stdout: C::s<int>:
//@ stdout: C::s<char>: 11
//@ stdout: C::s<long>: 2
//@ stdout: C::m<int>:
//@ stdout: C::m<long>: 3
//@ stdout: O<int>::g<int>:
//@ stdout: O<int>::g<char>: 12
//@ stdout: O<int>::g<long>: 4
//@ stdout: O<int>::h:
//@ stdout: O<char>::h: 5
//@ stdout: O<short>::h: 1 16
//@ stdout: O<long>::h: 1
//@ stdout: O<int>::sm: 2
//@ stdout: O<long>::sm: 2
//@ stdout: p<int> param: 6
//@ stdout: p<char> param: 6 7
//@ stdout: p<long> param: 6
//@ stdout: v<int>:
//@ stdout: v<char>: 13
//@ stdout: v<long>: 5
//@ stdout: S<int>:
//@ stdout: S<char>: 7
//@ stdout: S<long>: 1
#include <cstdio>
#include <meta>
struct tag { int v; };

// A namespace-scope function template.
template <class T> [[=tag{1}]] int f(T) { return 0; }
template <> [[=tag{8}]] int f<int>(int) { return 1; }
template <> int f<char>(char) { return 1; }
// Redeclarations of an explicit specialization: each one's, in order.
template <> [[=tag{9}]] int f<short>(short);
template <> [[=tag{10}]] int f<short>(short);
template <> [[=tag{11}]] int f<short>(short) { return 2; }
// Explicit instantiations: the pattern's, then their own.
template int f<long>(long);
template [[=tag{14}]] int f<unsigned>(unsigned);

// A value-dependent pattern annotation (substituted lazily).
template <class T> [[=tag{sizeof(T)}]] int dd(T) { return 0; }
template <> int dd<int>(int) { return 1; }
template <> [[=tag{20}]] int dd<char>(char) { return 1; }
extern template [[=tag{15}]] int dd<short>(short);

// Static and non-static member function templates.
struct C {
  template <class T> [[=tag{2}]] static int s(T) { return 0; }
  template <class T> [[=tag{3}]] int m(T) { return 0; }
};
template <> inline int C::s<int>(int) { return 1; }
template <> [[=tag{11}]] inline int C::s<char>(char) { return 1; }
template <> inline int C::m<int>(int) { return 1; }

// A member template of a class template, and members of one.
template <class T> struct O {
  template <class K> [[=tag{4}]] static int g() { return 0; }
  [[=tag{1}]] static int h() { return 0; }
  [[=tag{2}]] static int sm;
};
template <class T> int O<T>::sm = 0;
template <> template <> int O<int>::g<int>() { return 1; }
template <> template <> [[=tag{12}]] int O<int>::g<char>() { return 1; }
template <> int O<int>::h() { return 1; }
template <> [[=tag{5}]] int O<char>::h() { return 1; }
template [[=tag{16}]] int O<short>::h();
// A static data member's explicit specialization keeps the member's.
template <> int O<int>::sm = 1;

// A parameter of an explicit specialization keeps the pattern
// parameter's, then its own.
template <class T> int p([[=tag{6}]] T) { return 0; }
template <> int p<int>(int) { return 1; }
template <> int p<char>([[=tag{7}]] char) { return 1; }

// Variable templates.
template <class T> [[=tag{5}]] int v = 0;
template <> int v<int> = 1;
template <> [[=tag{13}]] int v<char> = 1;

// Class templates (already per-declaration).
template <class T> struct [[=tag{1}]] S {};
template <> struct S<int> {};
template <> struct [[=tag{7}]] S<char> {};

template <std::meta::info R> void show(const char* n) {
  static constexpr auto a = std::define_static_array(annotations_of(R));
  std::printf("%s:", n);
  template for (constexpr auto x : a) std::printf(" %d", std::meta::extract<tag>(x).v);
  std::printf("\n");
}

int main() {
  show<^^f<int>>("f<int>");
  show<^^f<char>>("f<char>");
  show<^^f<short>>("f<short>");
  show<^^f<long>>("f<long>");
  show<^^f<unsigned>>("f<unsigned>");
  show<^^f<double>>("f<double>");
  show<^^dd<int>>("dd<int>");
  show<^^dd<char>>("dd<char>");
  show<^^dd<short>>("dd<short>");
  show<^^dd<long>>("dd<long>");
  show<^^C::s<int>>("C::s<int>");
  show<^^C::s<char>>("C::s<char>");
  show<^^C::s<long>>("C::s<long>");
  show<^^C::m<int>>("C::m<int>");
  show<^^C::m<long>>("C::m<long>");
  show<^^O<int>::g<int>>("O<int>::g<int>");
  show<^^O<int>::g<char>>("O<int>::g<char>");
  show<^^O<int>::g<long>>("O<int>::g<long>");
  show<^^O<int>::h>("O<int>::h");
  show<^^O<char>::h>("O<char>::h");
  show<^^O<short>::h>("O<short>::h");
  show<^^O<long>::h>("O<long>::h");
  show<^^O<int>::sm>("O<int>::sm");
  show<^^O<long>::sm>("O<long>::sm");
  show<parameters_of(^^p<int>)[0]>("p<int> param");
  show<parameters_of(^^p<char>)[0]>("p<char> param");
  show<parameters_of(^^p<long>)[0]>("p<long> param");
  show<^^v<int>>("v<int>");
  show<^^v<char>>("v<char>");
  show<^^v<long>>("v<long>");
  show<^^S<int>>("S<int>");
  show<^^S<char>>("S<char>");
  show<^^S<long>>("S<long>");
}
