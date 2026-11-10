// UBSan type descriptors name each type as clang's diagnostics print it
// (`EmitCheckTypeDescriptor` -> `ConvertArgToString(ak_qualtype)`):
// a member access checks the written pointee (`getPointeeType` keeps its
// sugar: `'ns::Outer::Inner'`, `'ns::H<>'`, `'InnerT' (aka
// 'ns::Outer::Inner')`), a substituted template parameter prints fully
// scoped, an implicit specialization drops the trailing arguments equal
// to their defaults, and an explicit specialization or instantiation
// prints its arguments as written (`'std::coroutine_handle<void>'`,
// `'n::X<TA>'`; clang 19 takes a redeclared explicit specialization's
// arguments from its definition, clang 22 from its first declaration).
// Downcasts check the written target type, `delete` the written pointee.
// spec/codegen.md "Sanitizers", WP-1158.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=null,alignment,enum
namespace std {
template <class P = void> struct coroutine_handle;
template <> struct coroutine_handle<void> {
  void *p;
  void *address() const { return p; }
};
} // namespace std
namespace ns {
struct Outer {
  struct Inner {
    int x;
    int get() { return x; }
  };
  struct B {
    virtual ~B();
    int y;
  };
  struct D : B {};
};
template <class T = void, class U = int> struct H {
  int x;
  int get() { return x; }
};
typedef Outer::Inner InnerT;
typedef Outer::D DT;
using HD = H<>;
enum E { a, b };
typedef E myE;
template <class... T> struct P {
  int x;
};
} // namespace ns
namespace n {
struct A {};
typedef A TA;
template <class T, class U = int> struct X {
  int x;
};
template <> struct X<TA> {
  int x;
};
template <> struct X<long>;
template <> struct X<long, int> {
  int x;
};
template struct X<A *, int>;
} // namespace n

// Nested class, default template arguments, typedef and alias sugar.
int f1(ns::Outer::Inner *p) { return p->x; }
int f2(ns::H<> *p) { return p->x; }
int f3(ns::InnerT *p) { return p->x; }
int f4(ns::HD *p) { return p->get(); }
int f5(ns::H<char> *p) { return p->get(); }
int f6(ns::Outer::Inner **p) { return (*p)->x; }
// Enums (the loaded value's type).
int f7(ns::E *p) { return *p; }
int f8(ns::myE *p) { return *p; }
// Packs.
int f9(ns::P<int, char> *p) { return p->x; }
int f10(ns::P<> *p) { return p->x; }
// A lambda's closure type.
int f11() {
  auto l = [](int y) { return y; };
  auto *pl = &l;
  return (*pl)(1);
}
// Substituted template parameters print fully scoped.
template <class T> int get(T *p) { return p->x; }
int f12(ns::Outer::Inner *p) { return get(p); }
int f13(ns::H<> *p) { return get(p); }
// Explicit specializations and instantiations: the written arguments.
void *f14(std::coroutine_handle<> *p) { return p->address(); }
int f15(n::X<n::A> *p) { return p->x + get(p); }
int f16(n::X<long> *p) { return p->x + get(p); }
int f17(n::X<n::A *> *p) { return p->x + get(p); }
// Downcasts check the written target type; delete the written pointee.
int f18(ns::Outer::B *p) { return static_cast<ns::DT *>(p)->y; }
int f19(ns::Outer::B &r) { return static_cast<ns::DT &>(r).y; }
void f20(ns::Outer::Inner *p) { delete p; }
