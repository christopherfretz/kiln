// sides: pre
// scope: defs
// expect: differ _ZN2hp5gotosEi
// `[[noreturn]]` on the first declaration only ([dcl.attr.noreturn]: a
// unit without it is ill-formed, no diagnostic required): hashed from the
// declaration chain, so the row of `gotos` differs, nothing else.
namespace hp { [[noreturn]] inline int gotos(int n); }
