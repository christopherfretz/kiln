#pragma pack(push, outer, 2)
#pragma pack(push, 1)
#pragma pack(push, inner, 8)
struct A { char c; double d; };
#pragma pack(pop, outer)
struct B { char c; double d; };
#pragma pack(push, 4)
#pragma pack(pop, 2)
struct C { char c; double d; };
