// The written-reference table (spec/ast.md §18): the entry of an
// overloaded name is made where it is written and patched with the
// function overload resolution selects -- by the call's arguments, by
// argument-dependent lookup, by a target type for `&f`, a member
// function, a member function template specialization, and a function
// template specialization. An overloaded operator's use (`a + b`) has
// no written name and no entry.
//@ written-refs: overloads.refs
namespace lib {
struct Tag {};
int act(Tag);
int act(Tag, int);
Tag operator+(Tag, Tag);
} // namespace lib

int f(int);
int f(double);
template <class T> int f(T*);

struct Obj {
  int m(int);
  int m(char);
  static int s(int);
  static int s(long);
  template <class T> int t(T);
};

int calls(Obj o, Obj* p, lib::Tag g) {
  int (*fp)(double) = &f;
  int (Obj::*mp)(char) = &Obj::m;
  int r = f(1) + f(2.0) + f(&r) + fp(0);
  r += o.m(1) + p->m('c') + o.s(1L) + Obj::s(1) + o.t(1.0) + (o.*mp)('x');
  return r + act(g) + act(g + g, r);
}
