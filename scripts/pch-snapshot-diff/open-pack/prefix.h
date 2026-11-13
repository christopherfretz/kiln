// Ends with #pragma pack(push, 1) open; the main file's structs are
// packed until its pop.
struct PfxBefore { char c; int i; };
#pragma pack(push, 1)
struct PfxInside { char c; int i; };
