//@ expect: pp
// #pragma push_macro/pop_macro save and restore a definition (or its
// absence); the pragma lines themselves are kept in the output.
#define X 1
X
#pragma push_macro("X")
#undef X
#define X 2
X
#pragma push_macro("X")
#undef X
X
#pragma pop_macro("X")
X
#pragma pop_macro("X")
X
#pragma push_macro("NEVER_DEFINED")
#define NEVER_DEFINED 3
NEVER_DEFINED
#pragma pop_macro("NEVER_DEFINED")
NEVER_DEFINED
