#pragma pack(push, 2)
struct K { char c; double d; int x : 3; };
struct M { char c; long long a : 40; int b : 30; };
#pragma pack(pop)
struct N { char c; long long a : 40; };
