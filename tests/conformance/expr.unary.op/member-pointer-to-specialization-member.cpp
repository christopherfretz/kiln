// [expr.unary.op]/3: `&C::m` of a non-static data member `m` of type `T` is
// a prvalue of type "pointer to member of class C of type T", whatever `T`
// is. When `T` is a class template specialization whose associated classes
// are not yet instantiated, forming the pointer must not complete them as if
// `&` were an overloadable operator on an object of class type: the operand
// is never an object expression ([over.match.oper]/1 does not apply; clang's
// `isQualifiedMemberAccess` in `BuildUnaryOp`), so no `operator&` is looked
// up and no instantiation runs while the name `C::m` is still pending. The
// shape is nanobind's `class_<T>::def_rw("name", &::ns::T::member)` as
// generated binding code writes it, for a member of type `std::map<int, int>`: the
// lookup of `operator&` for `std::map<int, int, less<int>,
// allocator<pair<const int, int>>>` instantiated `std::pair<const int, int>`,
// whose `explicit(...)` constant expression ended the pending `&S::m` as a
// use without an object. `Assoc<T>` below stands in for that `pair`: only
// argument-dependent lookup reaches it, and its instantiation evaluates a
// constant expression. Each use names a fresh specialization.
//@ expect: run
//@ strict-diags
template <int N> struct Tag { static constexpr int id = N; };
template <class T> struct Assoc { static_assert(T::id > 0); };
template <class T, class A = Assoc<T>> struct Box { static constexpr int id = T::id; };
// An `operator&` of the member's class is not called either.
struct Amp {
  static constexpr int id = 9;
  int operator&() const { return 99; }
};
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };

namespace ns {
struct Outer {
  struct S {
    static constexpr int tag = 100;
    Box<Tag<1>> a;
    Box<Tag<2>> b;
    Box<Tag<3>> c;
    Box<Tag<4>> d;
    Box<Tag<5>> e;
    Box<Tag<6>> g;
    Amp amp;
    int fn(Box<Tag<7>>) { return 7; }
    int self();
  };
};
}  // namespace ns
using ns::Outer;

template <class C, class D> int f(D C::*) { return C::tag + D::id; }
template <class C, class R, class A> int h(R (C::*)(A)) { return C::tag + A::id; }
template <auto P> int nttp() { return f(P); }
template <class P> int via_type() { return same<P, Box<Tag<5>> Outer::S::*>::value ? 5 : 50; }
struct Binder {
  template <class C, class D> int def_rw(const char*, D C::*p) { return f(p); }
};

static_assert(same<decltype(&Outer::S::d), Box<Tag<4>> Outer::S::*>::value);

// In a member function of the class, `&S::g` still forms a pointer to member
// rather than `&this->S::g`.
int Outer::S::self() { return f(&Outer::S::g); }

int main() {
  Binder cls;
  if (f(&Outer::S::a) != 101) return 1;
  if (cls.def_rw("b", &::ns::Outer::S::b) != 102) return 2;
  if (nttp<&Outer::S::c>() != 103) return 3;
  if (via_type<decltype(&Outer::S::e)>() != 5) return 5;
  if (Outer::S{}.self() != 106) return 6;
  if (h(&Outer::S::fn) != 107) return 7;
  if (f(&Outer::S::amp) != 109) return 9;
  Amp x;
  return &x == 99 ? 0 : 10;
}
