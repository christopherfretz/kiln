#define STR2(x) #x
#define STR(x) STR2(x)
inline const char *f1() { return STR(__FILE__); }
inline const char *f2() { return "at " __FILE__; }
