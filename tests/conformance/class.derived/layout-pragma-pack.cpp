// [cpp.pragma] (implementation-defined behaviour), [class.derived],
// Itanium C++ ABI §2.4 with the GCC/Clang `#pragma pack` extension: packing
// caps the alignment of members and bases declared while it is in effect.
//@ expect: layout
struct B { char c; int i; };              // unpacked: i at 4
#pragma pack(push, 1)
struct P1 : B { char d; double e; };      // e at 9, alignment 1
struct P2 { char c; B b; short s; };
#pragma pack(2)
struct P3 : B { char d; int e; long long f; };
#pragma pack(pop)
struct U : P1 { char g; };                // unpacked, but P1's alignment is 1
P1 p1; P2 p2; P3 p3; U u;
