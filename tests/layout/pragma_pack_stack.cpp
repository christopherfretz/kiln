#pragma pack(push, 1)
struct A { char c; int i; };
#pragma pack(push, 4)
struct B { char c; double d; };
#pragma pack(pop)
struct C { char c; int i; };
#pragma pack(pop)
struct D { char c; int i; };
