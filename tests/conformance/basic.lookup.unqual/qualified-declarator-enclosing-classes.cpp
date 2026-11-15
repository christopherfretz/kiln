// [basic.scope.class]/1, [basic.lookup.unqual]/3, [class.access.general]/7:
// names in the declarator of an out-of-line member definition after its
// qualified declarator-id (parameters, default arguments, noexcept-specifier,
// requires-clause, trailing return type, the initializer of a static data
// member) are looked up in the member's class and so through its enclosing
// classes, with access checked as for the member: private, protected and
// public middle classes, one to three levels, the class itself, an
// enclosing class, a sibling nested class and nested typedefs, aliases and
// enumerations of an enclosing class, for constructors, a destructor, a
// conversion function, operators, member functions, member templates,
// static data members and members of a class template (WP-1455).
//@ expect: compile-ok
// one level, protected
class O1 {
protected:
  struct R {
    R(int);
    R(R &&);
    R(const O1 &, R *);
  };
};
O1::R::R(int) {}
O1::R::R(R &&) {}
O1::R::R(const O1 &, R *) {}

// two levels, private / public middle classes
class O2 {
  struct I {
    typedef int T;
    enum E { e0 };
    static constexpr int k = 1;
    struct R;
    using A = R;
    struct R {
      R(I::R &&);
      R(I *, const R &);
      ~R();
      operator I::R *();
      R &operator=(I::R &&);
      bool operator==(const I::R &) const;
      void f(I::R, I::E = I::e0, I::T = I::k);
      static I::T x;
      static const int k2;
      int g(I::T) noexcept(I::k == 1);
      auto h() -> I::R;
      auto h2() -> R;
      template <class U> R(U, I::T);
      template <class U> void m(U, I::R &);
      template <class U> void c(U, I::T) requires (I::k == 1);
    };
  };
  struct S { };
public:
  struct P {
    struct R { R(P::R &&); R(S, I); };
  };
};
O2::I::R::R(I::R &&) {}
O2::I::R::R(I *, const R &) {}
O2::I::R::~R() {}
O2::I::R::operator I::R *() { return this; }
O2::I::R &O2::I::R::operator=(I::R &&) { return *this; }
bool O2::I::R::operator==(const I::R &) const { return true; }
void O2::I::R::f(I::R, I::E, I::T) {}
O2::I::T O2::I::R::x = I::k;
const int O2::I::R::k2 = I::k + sizeof(I::R);
int O2::I::R::g(I::T) noexcept(I::k == 1) { return 0; }
auto O2::I::R::h() -> I::R { return static_cast<I::R &&>(*this); }
auto O2::I::R::h2() -> R { return static_cast<A &&>(*this); }
template <class U> O2::I::R::R(U, I::T) {}
template <class U> void O2::I::R::m(U, I::R &) {}
template <class U> void O2::I::R::c(U, I::T) requires (I::k == 1) {}
O2::P::R::R(P::R &&) {}
O2::P::R::R(S, I) {}

// three levels; sibling nested class, alias, enum of an enclosing class
class O3 {
protected:
  struct A {
    using T = long;
    enum class E { x };
    struct Sib { };
  private:
    struct B {
      struct R {
        R(A::B::R &&, Sib, B *);
        R(A::T, A::E, O3 *);
        R(B::R &, A::Sib);
      };
    };
  };
};
O3::A::B::R::R(A::B::R &&, Sib, B *) {}
O3::A::B::R::R(A::T, A::E, O3 *) {}
O3::A::B::R::R(B::R &, A::Sib) {}

// class templates and member templates
template <class X> class OT {
protected:
  struct I {
    struct R {
      R(I::R &&);
      template <class U> R(U *, I::R &);
      static int s;
    };
  };
};
template <class X> OT<X>::I::R::R(I::R &&) {}
template <class X> template <class U> OT<X>::I::R::R(U *, I::R &) {}
template <class X> int OT<X>::I::R::s = sizeof(I::R);

int main() {
  return 0;
}
