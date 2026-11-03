// Clang ext vectors, [clang 20+]: a bool ext vector condition selects lane by
// lane on its `<N x i1>` lanes (the packed i8 loaded, bit-cast to `<8 x i1>`
// and shuffled down to the four lanes), lowered to a single `select`. WP-187.
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=x86_64-linux-gnu
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef int v4i __attribute__((ext_vector_type(4)));
v4i sel(b4 m, v4i x, v4i y) { return m ? x : y; }
b4 mk(v4i x, v4i y) { return __builtin_convertvector(x < y, b4); }
b4 ops(b4 a, b4 b) { return (a & b) | ~a; }
