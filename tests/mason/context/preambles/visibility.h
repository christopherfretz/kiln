// sides: pre post
// scope: any
// `#pragma GCC visibility` around unrelated declarations.
#pragma GCC visibility push(hidden)
inline int kc_hidden() { return 2; }
struct KcVis { int f() { return 1; } };
#pragma GCC visibility pop
