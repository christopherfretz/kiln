struct __attribute__((packed)) P { char c; int a : 20; int b : 12; };
struct Q { char c; int a : 20 __attribute__((packed)); };
