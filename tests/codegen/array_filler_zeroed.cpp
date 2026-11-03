// [dcl.init.aggr]/5, [dcl.init.general]/9: an array zeroed by one `memset`
// keeps that state through its filler loop (clang `EmitArrayInit` passes
// `Dest.isZeroed()` to each element): no per-element `memset` and no zero
// stores for the members the filler value-initializes.
struct D { int x = 1; int y; };
struct Ctor { Ctor(); int v; };
struct HasCtor { Ctor c; int z; };
struct Agg2 { D d; int n; };
struct Big { int x = 1; int y[8]; };
struct Big2 { int x; int y[8]; };
void small() { D a[3] = {}; }
void some() { D a[8] = {{2, 3}}; }
void ctor() { HasCtor a[3] = {}; }
void nested() { Agg2 a[3] = {}; }
void nested2() { Agg2 a[2] = {}; }
void big() { Big a[2] = {}; }
void big_partial() { Big2 a[2][3] = {{{1}}}; }
void big_2d() { Big a[2][3] = {}; }
