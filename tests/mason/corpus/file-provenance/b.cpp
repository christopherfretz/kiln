inline const char *f() { return "in " __FILE__; }
const char *(*keep_b)() = f;
