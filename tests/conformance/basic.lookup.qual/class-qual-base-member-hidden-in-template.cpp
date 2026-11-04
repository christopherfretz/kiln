// [class.qual]/1, [basic.lookup.qual.general]/1, [temp.dep.type]/1: in a
// member of a class template, the qualified-id `Base::m` whose
// nested-name-specifier names a base class looks `m` up in that base only;
// the derived class's own member `m()` never hides it, whether the access
// is the implicit `(*this).Base::m`, an explicit `this->Base::m`, `&Base::m`
// or the base is dependent (`Dep<T>::m`). libstdc++'s `_State<_CharT>::
// _M_opcode()` returns `_State_base::_M_opcode` (WP-608).
//@ expect: run
//@ strict-diags
//@ stdout: 7 7 7 8 9
#include <cstdio>

struct Base {
  int m;
};

template <class T> struct Dep {
  int n;
};

template <class T> struct Derived : Base, Dep<T> {
  Derived() : Base{7}, Dep<T>{8} {}
  int m() const { return Base::m; }
  int m2() const { return this->Base::m; }
  int m3() const { return this->*&Base::m; }
  int n() const { return Dep<T>::n; }
  int k = 9;
  int k2() const { return Derived::k; }
};

int main() {
  Derived<char> d;
  std::printf("%d %d %d %d %d\n", d.m(), d.m2(), d.m3(), d.n(), d.k2());
  if (d.m() != 7 || d.m2() != 7 || d.m3() != 7)
    return 1;
  return d.n() == 8 && d.k2() == 9 ? 0 : 2;
}
