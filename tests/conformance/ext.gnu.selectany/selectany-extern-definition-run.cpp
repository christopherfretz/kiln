// GNU `selectany` on an `extern` declaration without an initializer
// (clang `VarDecl::isThisDeclarationADefinition`: a declaration with its
// own, not inherited, `SelectAnyAttr` is a definition; [basic.def]/2
// otherwise makes it a declaration). Both units define `f` and `obj`
// (`weak_odr`, in a comdat), so the program links, the linker keeps one
// of each, and the dynamic initializer runs once. A later `extern int g;`
// after `extern selectany int g;` inherits the attribute and only
// redeclares; an earlier plain `extern` is completed by the selectany
// one (WP-1745).
//@ expect: run
//@ strict-diags
//@ args: %{test_dir}/Inputs/selectany-extern-other.cpp
//@ stdout: same 1 1
//@ stdout: values 0 0 0 1 1
#include <cstdio>
#include "Inputs/selectany-extern.h"
int counted2_runs = 0;
Counted2::Counted2() : v(++counted2_runs) {}
int *other_f();
Counted2 *other_obj();
extern __attribute__((selectany)) int g;
extern int g;
extern int h;
extern __attribute__((selectany)) int h;
int main() {
  std::printf("same %d %d\n", other_f() == &f, other_obj() == &obj);
  std::printf("values %d %d %d %d %d\n", f, g, h, obj.v, counted2_runs);
}
