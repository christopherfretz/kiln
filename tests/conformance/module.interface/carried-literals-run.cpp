// [lex.string], [class.temporary]/6, [expr.const], [module.interface],
// spec/modules.md §11.11 Part 2 (WP-1160): an importer uses constexpr
// pointers to string literals, lifetime-extended temporaries and labels
// (a GNU extension) the module evaluated. A kind-2 module file carries a
// string literal or label address as the position of its expression and
// a temporary as its extending declaration, mangling index and value, so
// the importer's pointers designate its own objects, with the module's
// values, as with kind 1 (`--bmi-kind 1|2`): `ps == s1`, `tb == &tp.b`.
//@ expect: run
//@ std: c++20
//@ module: cl=Inputs/cl.cppm
//@ stdout: abc cd 120 42 3 4 9 tmpl tmpl fu 1 2 1
import cl;
extern "C" int printf(const char *, ...);
static_assert(s1[0] == 'a' && s1[3] == 0 && *s2 == 'c' && ps == s1 && s8[1] == u8'y');
static_assert(t1 == 42 && tp.a == 3 && tb == &tp.b && *tb == 4 && rr.r == 9);
static_assert(n1[0] == 't' && n2[3] == 'l');
// The temporaries are read in constant expressions only: clang gives a
// module's lifetime-extended temporaries internal linkage in its object
// file, so an importer's run-time reference to one does not link.
constexpr int v[] = {t1, tp.a, *tb, rr.r};
int main() {
  int same = ps == s1 && fu() == fu();
  printf("%s %s %d %d %d %d %d %s %s %s %d %d %d\n", s1, s2, s8[0], v[0], v[1], v[2], v[3], n1,
         n2, fu(), lab(0), lab(1), same);
  return same ? 0 : 1;
}
