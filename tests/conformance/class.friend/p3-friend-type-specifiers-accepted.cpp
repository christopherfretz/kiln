// [class.friend]/3, [class.friend]/1: `friend` first, then a simple-,
// elaborated- or typename-specifier; members befriended by qualified name.
//@ expect: compile-ok
//@ strict-diags
class c;
template<class> struct S { struct X {}; };
struct B { ~B(); operator int(); B(); };
struct A {
  friend class c;
  friend S<int>;
  friend c;
  template<class U> friend struct S;
  friend B::operator int();
  friend B::~B();
  friend B::B();
  friend inline void f() {}
  friend constexpr int g() { return 0; }
};
