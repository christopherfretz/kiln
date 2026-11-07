// [class.base.init]/2, [temp.res.general]/1: in a constructor of a member class of a class
// template, the unqualified mem-initializer-id `allocator_type` names the enclosing class's
// typedef, which the class also uses as its (dependent) base. Lookup at the definition does not
// search the dependent base, so the name is bound to the typedef there; the instantiation's base
// `Alloc<char>`, which has a static member function named `allocator_type`, does not hide it
// (libstdc++'s `basic_string::_Alloc_hider` with oneTBB's `tbb_allocator`; WP-803).
//@ expect: run
//@ strict-diags
//@ stdout: 3 5 7
#include <cstdio>
template <class T> struct Alloc {
  static int allocator_type() { return 7; }
  int v = 0;
  Alloc(int x) : v(x) {}
};
template <class C, class A> struct Str {
  typedef A allocator_type;
  struct Hider : allocator_type {
    int p;
    Hider(int d, const A& a) : allocator_type(a), p(d) {}
  };
  Hider h;
  Str(int d, const A& a) : h(d, a) {}
};
int main() {
  Str<char, Alloc<char>> s(3, Alloc<char>(5));
  std::printf("%d %d %d\n", s.h.p, s.h.v, s.h.allocator_type());
}
