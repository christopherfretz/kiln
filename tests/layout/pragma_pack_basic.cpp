#pragma pack(1)
struct P1 { char c; int i; double d; };
#pragma pack(2)
struct P2 { char c; int i; double d; };
#pragma pack(4)
struct P4 { char c; double d; };
#pragma pack()
struct P0 { char c; double d; };
