//@ expect: ast
// `__builtin_LINE()`: clang's SourceLocExpr is not in the M2 dump, so the
// dump cannot be normalized.
unsigned n = __builtin_LINE();
