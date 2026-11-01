//@ expect: ast
//@ ast-filter: P
// [temp.param], [temp.explicit]: clang's -ast-dump-filter names a class
// template's parameter by the class (`n::SS::PK`), a function template's
// unqualified (the function is its context), and an alias template's,
// variable template's or concept's by the enclosing namespace. It
// searches a template's pattern (`ff`'s parameter and local) but not an
// explicit instantiation's members (`SS<int>::Pd`).
namespace n {
template <class PA> using AL = PA *;
template <class PV> constexpr int VV = 0;
template <class PC> concept CC = true;
template <class PF> int ff(PF Pp) { int Pl = 0; return Pl; }
template <class PK> struct SS { template <class PM> void mm(); int Pd; };
template struct SS<int>;
}
