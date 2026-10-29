struct __attribute__((packed, aligned(4))) P { char c; int i; };
struct __attribute__((packed)) Q { char c; int i __attribute__((aligned(2))); };
