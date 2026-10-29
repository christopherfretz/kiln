struct __attribute__((packed)) P { char c; int i; double d; };
struct Q { char c; P p; };
struct __attribute__((packed)) R { char c; P p; short s; };
