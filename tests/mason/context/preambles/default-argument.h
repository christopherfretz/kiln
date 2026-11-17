// sides: pre post
// scope: defs
// expect: differ _ZN2hp3lamEi
// A default argument added by a redeclaration: the accumulated defaults
// at the end of the unit differ, which [dcl.fct.default] makes ill-formed
// across units (mortar's K8): the row of `lam` differs, nothing else.
namespace hp { inline int lam(int y = 5); }
