// A namespace-scope anonymous union's variable is mangled with the name of
// the union's first named data member, through nested anonymous records,
// and without the internal-linkage `L` (clang `mangleUnqualifiedName`).
static union { int ga; float gb; };
static union { struct { int : 3; int hb; }; int hc; };
namespace n { static union { int na; }; }
namespace { union { int ua; }; }
int rd() { return ga + hb + hc + n::na + ua; }
