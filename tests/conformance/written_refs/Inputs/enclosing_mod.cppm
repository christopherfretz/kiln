// The module unit of enclosing-module-image.cpp and
// enclosing-module-replay.cpp: the holders of spec/ast.md §18.3 and the
// declarator groups of §18.3.1, carried by the image.
export module enclosing_mod;
export namespace shapes {
struct Pt { int x, y; };
using Alias = Pt;
enum Kind { small = sizeof(Pt) };
template <class T = Pt> T make() { return T{}; }
static_assert(sizeof(Pt) == 2 * sizeof(int));
Pt first, second;
struct Pair { Pt a, b; };
typedef Pt T1, T2;
} // namespace shapes
