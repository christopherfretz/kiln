//@ args: -O1
// `!tbaa.struct` on aggregate copies at -O1 ([class.copy.ctor]/14,
// [class.copy.assign]/12, [basic.lval]/11; clang `EmitAggregateCopy`): the
// `llvm.memcpy` of a trivial copy lists each scalar field as `(offset,
// size, tag)`, nested classes flattened; a union, an array or a vector
// member is one "omnipotent char" entry of its size, a run of bit-fields
// one "omnipotent char" entry of its storage size; a class with a base
// or a flexible array member gets none. A memberwise copy constructor's
// field-range memcpy gets none.
#define ON __attribute__((optnone))
struct In { short s; int i; };
struct S { int a; long b; In in; char c; double d; int* p; bool bo; int arr[3]; };
union U { int i; float f; };
struct WU { int x; U u; };
struct BF { char k; int a : 3; int b : 5; int c; };
struct Base { int b0; };
struct Der : Base { int d0; };
typedef int v4 __attribute__((vector_size(16)));
struct VS { int a; v4 v; };
struct E {};
struct NU { int a; [[no_unique_address]] E e; int b; };
ON void copy_assign(S* a, S* b) { *a = *b; }
ON void copy_union(WU* a, WU* b, U* c, U* d) { *a = *b; *c = *d; }
ON void copy_bitfields(BF* a, BF* b) { *a = *b; }
ON void copy_derived(Der* a, Der* b) { *a = *b; }
ON void copy_vector(VS* a, VS* b) { *a = *b; }
ON void copy_nua(NU* a, NU* b) { *a = *b; }
ON void copy_ctor(In* a, In b) { *a = b; }
ON S copy_construct(S* p) { return *p; }
