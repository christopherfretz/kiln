// [expr.typeid]/3, [basic.def.odr]/4-5, [temp.inst]/5: the operand of
// `typeid` that is a glvalue of polymorphic class type is evaluated, so it
// is potentially evaluated and odr-uses what it names: an implicitly
// instantiated member function (`P<B>::operator*`), an inline function,
// a lambda's call operator, an implicit copy assignment, a conversion
// function of a class template specialization, and the destructor of a
// temporary (destroyed at the end of the enclosing full-expression). In a
// function template the operand is polymorphic or not per instantiation:
// `Q<NP>::operator*`, whose body cannot be instantiated, is named only in
// an unevaluated operand. A non-polymorphic operand stays unevaluated:
// `undefined_np` is never defined. WP-1322 (the reference workload): kiln
// referenced `P<B>::operator*` without instantiating it, a link failure.
//@ expect: run
//@ std: c++20
//@ args: -Wno-potentially-evaluated-expression
//@ stdout: 1 1 1 1 1 1
//@ stdout: ~Tmp
//@ stdout: after 1 1 1
#include <cstdio>
#include <type_traits>
#include <typeinfo>
struct B { virtual ~B() {} };
struct D : B {};
struct NP { int x; };
template <class T> struct P {
  T *p;
  T &operator*() const { return *p; }
};
template <class T> struct Q {
  T *p;
  T &operator*() const {
    static_assert(std::is_polymorphic_v<T>, "instantiated");
    return *p;
  }
};
template <class T> struct W {
  T *p;
  operator T &() const { return *p; }
};
struct Tmp {
  B *b;
  ~Tmp() { std::puts("~Tmp"); }
  B &get() const { return *b; }
};
struct V { virtual ~V() {} int k = 0; };
struct VD : V {};
D d;
P<B> &held() { static P<B> h{&d}; return h; }
inline B &inl() { return d; }
NP undefined_np();
template <class T> bool tf(const Q<T> &q, const std::type_info &want) {
  return typeid(*q) == want;
}
int main() {
  VD a, b;
  W<B> w{&d};
  std::printf("%d %d %d %d %d %d\n", typeid(*held()) == typeid(D),
              typeid(inl()) == typeid(D),
              typeid([]() -> B & { return d; }()) == typeid(D),
              typeid(a = b) == typeid(VD),
              typeid(static_cast<B &>(w)) == typeid(D),
              typeid(Tmp{&d}.get()) == typeid(D));
  Q<B> qb{&d};
  Q<NP> qn{nullptr};
  std::printf("after %d %d %d\n", tf(qb, typeid(D)), tf(qn, typeid(NP)),
              typeid(undefined_np()) == typeid(NP));
}
