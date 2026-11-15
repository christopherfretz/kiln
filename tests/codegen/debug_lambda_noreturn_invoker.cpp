// Debug info and IR ([expr.prim.lambda.closure]/8, [dcl.attr.noreturn]):
// the static invoker `__invoke` of a captureless lambda converted to a
// function pointer has the call operator's function type. The GNU
// `__attribute__((noreturn))` is part of that type, so the invoker is
// `noreturn` in the IR and DIFlagNoReturn in its DISubprograms; the C++11
// `[[noreturn]]` appertains to the call operator only, and the invoker
// has neither, as clang 19 and 22 emit it. Either way its call of the
// call operator is a `noreturn` call followed by `unreachable`. The
// invoker is private (clang `addFunctionPointerConversion`), so its
// declaration has no access flag.
//@ args: -g -Wno-invalid-noreturn
void (*p)() = [] [[noreturn]] () { throw 1; };
void (*q)() = [] () __attribute__((noreturn)) { throw 1; };
// A function whose type is `noreturn` ends in `unreachable` where it would
// return (clang `EmitFunctionEpilog`, `FI.isNoReturn()`).
void fall() __attribute__((noreturn));
void fall() {}
