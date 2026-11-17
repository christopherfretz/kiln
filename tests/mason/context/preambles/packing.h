// sides: pre post
// scope: any
// `packed` and `#pragma pack` on unrelated types.
struct __attribute__((packed)) KcPacked { char c; int i; };
#pragma pack(push, 1)
struct KcPacked2 { char c; long l; };
#pragma pack(pop)
