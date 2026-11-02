// [temp.local]/1, [temp.arg.template]/1: inside a class template's own
// definition, a name qualified by a namespace (or by the class the
// template is a member of) finds the template itself, not the
// injected-class-name, and is a template template argument. libstdc++
// bits/valarray_after.h:179 (`_Expr<_UnClos<__negate, std::_Expr, _Clos>,
// value_type>` in `_Expr`); <valarray> (docs/status/libstdcxx.md, WP-332).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 2 2 2
#include <stdio.h>
namespace ns {
template<class Op, template<class, class> class Meta, class Dom> struct UnClos {
  static constexpr int arity = 2;
};
template<class C, class T> class Expr {
public:
  Expr<UnClos<int, ns::Expr, C>, T> neg() const { return {}; }
  int n() const { return UnClos<int, ::ns::Expr, C>::arity; }
};
struct S {
  template<class, class> struct E {
    int n() const { return UnClos<int, S::E, int>::arity; }
  };
};
} // namespace ns

int main() {
  ns::Expr<int, long> e;
  ns::Expr<ns::UnClos<int, ns::Expr, int>, long> f = e.neg();
  printf("%d %d %d\n", e.n(), f.n(), ns::S::E<int, int>().n());
}
