#pragma pack(push, 2)
struct A { char c; alignas(8) int i; };
struct B { char c; int i __attribute__((aligned(8))); };
struct alignas(16) C { char c; int i; };
#pragma pack(pop)
