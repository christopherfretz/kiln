// A '#pragma clang attribute' region in the prefix: its declarations
// carry the attribute into the main file; an empty push with an attribute
// added later, and a namespaced group.
#pragma clang attribute push (__attribute__((annotate("pfx"))), apply_to = function)
inline int pfx_annotated() { return 1; }
#pragma clang attribute pop
#pragma clang attribute NS.push (__attribute__((cold)), apply_to = function)
#pragma clang attribute push
#pragma clang attribute (__attribute__((annotate("inner"))), apply_to = variable(is_global))
int pfx_var = 2;
inline int pfx_cold() { return 3; }
#pragma clang attribute pop
#pragma clang attribute NS.pop
