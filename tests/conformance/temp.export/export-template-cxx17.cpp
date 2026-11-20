// [diff.cpp03.temp], C++03 [temp]/1, [temp]/6: before C++20 `export` may start
// a template-declaration; the keyword is accepted and ignored, with clang's
// "exported templates are unsupported" warning (also after an enclosing template
// head, on an explicit specialization and in a namespace). WP-1744.
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
//@ diag: 14:1 warning "exported templates are unsupported"
//@ diag: 16:1 warning "exported templates are unsupported"
//@ diag: 22:20 warning "exported templates are unsupported"
//@ diag: 23:1 warning "exported templates are unsupported"
//@ diag: 25:1 warning "exported templates are unsupported"
//@ diag: 26:1 warning "exported templates are unsupported"
export template <class T> T id(T t) { return t; }
namespace N {
export template <class T> struct Box { T v; };
}
template <class T> struct Outer {
  template <class U> U get(U u);
  static int s;
};
template <class T> export template <class U> U Outer<T>::get(U u) { return u; }
export template <class T> int Outer<T>::s = 3;
template <class T> int f2(T) { return 1; }
export template <> int f2<int>(int) { return 2; }
export template <> struct Outer<char> { };
int use() {
  N::Box<int> b = {id(2)};
  return b.v + Outer<int>().get(1) + Outer<int>::s + f2(1);
}
