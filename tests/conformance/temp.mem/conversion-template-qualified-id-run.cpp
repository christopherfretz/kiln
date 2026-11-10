// [temp.mem]/6, [expr.prim.id.qual]/1, [class.conv.fct]: a specialization
// of a conversion function template is named by a conversion-function-id
// as a non-template conversion function to the same type is, so the
// qualified-id `S::operator int*` and, in a member function of S, the
// unqualified `operator int*` name the specialization deduced for `int*`
// ([temp.deduct.conv]): in `&S::operator int*`, in a call through a
// derived class's qualified name, and unqualified. Before WP-1246 only
// the member-access form (`s.operator int*()`) considered the templates;
// the id-expression forms were "no member named 'operator int *'". As for
// member access (and clang), only the named class's own templates are
// considered, not its bases'.
//@ expect: run
//@ strict-diags
//@ stdout: 1 1 1 1 0
#include <cstdio>

int g;
struct S {
  template <class T> operator T*() const { return reinterpret_cast<T*>(&g); }
  int* own() const { return operator int*(); }
  int* (S::*pm() const)() const { return &S::operator int*; }
};
struct D : S {
  int* base() const { return S::operator int*(); }
};

int main() {
  S s;
  D d;
  int* (S::*pm)() const = &S::operator int*;
  long* (S::*pl)() const = &S::operator long*;
  std::printf("%d %d %d %d %d\n", (s.*pm)() == &g, s.own() == &g, (s.*s.pm())() == &g,
              d.base() == &g, (void*)(s.*pl)() == nullptr);
}
