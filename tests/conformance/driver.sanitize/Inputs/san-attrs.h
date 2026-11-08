// The functions and globals the sanitizer attribute IR tests check (WP-675).
int g = 1;
int h;
__attribute__((no_sanitize("address"))) int ng = 2;
extern int ext;
int dyn = ext + 1;
void use(int*);
int f(int x) { int a[4]; use(a); return a[x] + g; }
__attribute__((no_sanitize("address"))) int f2(int x) { return x; }
__attribute__((disable_sanitizer_instrumentation)) int f3(int x) { return x; }
__attribute__((no_sanitize_address)) int f4(int x) { return x; }
const char* s() { return "hi"; }
