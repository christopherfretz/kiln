// [class.derived]/7 note, [intro.object]/9 (distinct objects of the same type
// have distinct addresses), [dcl.attr.nouniqueaddr], Itanium C++ ABI §2.4 II
// (empty base optimization: an empty base goes at offset 0 unless that would
// place two subobjects of the same type at the same address).
//@ expect: layout
struct E {};
struct F {};
struct S1 : E { int i; };                 // E at 0, shares the address of i
struct S2 : E, F { char c; };             // both empty bases at 0
struct S3 : E { E e; int i; };            // E-typed member conflicts: e at 1
struct S4 : E, S1 { char c; };            // E vs S1's E: S1 moves to 4
struct S5 { [[no_unique_address]] E e; int i; };
S1 s1; S2 s2; S3 s3; S4 s4; S5 s5;
