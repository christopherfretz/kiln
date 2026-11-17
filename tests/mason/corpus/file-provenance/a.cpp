// a literal with a __FILE__ piece hashes as one placeholder (X17)
inline const char *f() { return "at " __FILE__; }
const char *(*keep_a)() = f;
