// [module.global.frag]/1, [basic.def.odr]/15, [temp.expl.spec]/1,
// [temp.spec.partial.general]/1: an importer and the global module
// fragment of the module it imports define the same class, the same
// class template and the same explicit and partial specializations of it,
// in different text. Namespace p's are defined by the importer before
// its `import` (kiln builds gsm in-process after them: they belong to
// another translation unit, which gsm does not see), namespace q's after
// it (gsm's are hidden GMF declarations). Each is one entity: no
// redefinition (spec/modules.md §6, WP-1156).
//@ expect: compile-ok
//@ std: c++20
//@ module: gsm=Inputs/gmf-spec-merge.cppm
namespace p {
struct C { int x = 1; };
template <class T> struct L { static constexpr int s = 0; };
template <> struct L<float> { static constexpr int s = 1; };
template <class T> struct L<T *> { static constexpr int s = 2; };
}
import gsm;
namespace q {
template <class T> struct L { static constexpr int s = 0; };
template <> struct L<double> { static constexpr int s = 3; };
template <class T> struct L<T &> { static constexpr int s = 4; };
}
static_assert(gsm_get() == 1 + 1 + 2 + 3 + 4);
static_assert(p::C{}.x == 1 && p::L<float>::s == 1 && p::L<char *>::s == 2);
static_assert(p::L<int>::s == 0 && q::L<double>::s == 3 && q::L<long &>::s == 4);
int main() {}
