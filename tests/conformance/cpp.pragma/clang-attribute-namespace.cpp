// [cpp.pragma]: 'NS.push' and 'NS.pop' open and close independent
// '#pragma clang attribute' groups, popped in any order; '(attr, apply_to
// = rules)' after an empty push adds to the innermost group; a C++11
// specifier pushes each of its attributes (clang
// ActOnPragmaAttributeEmptyPush, ActOnPragmaAttributePop). WP-1399.
//@ expect: ir
#pragma clang attribute A.push (__attribute__((annotate("a"))), apply_to = function)
#pragma clang attribute B.push (__attribute__((cold)), apply_to = function)
inline int both() { return 1; }
#pragma clang attribute A.pop
inline int only_b() { return 2; }
#pragma clang attribute B.pop
inline int neither() { return 3; }

#pragma clang attribute push
#pragma clang attribute (__attribute__((annotate("x"))), apply_to = function)
#pragma clang attribute (__attribute__((cold)), apply_to = function)
inline int two() { return 4; }
#pragma clang attribute pop

#pragma clang attribute push ([[gnu::cold, clang::annotate("cxx11")]], apply_to = function)
inline int spelled_cxx11() { return 5; }
#pragma clang attribute pop

int use() { return both() + only_b() + neither() + two() + spelled_cxx11(); }
