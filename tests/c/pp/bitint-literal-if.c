// WP-1331: in #if a `wb` literal is intmax_t and a `uwb` literal
// uintmax_t (clang 19/22), so `1uwb - 2` is positive and `-1wb` negative.
//@ std: c23
//@ expect: pp
#if 1wb && -1wb < 0
signed
#endif
#if 1uwb - 2 > 0
unsigned
#endif
#if 0x7fffffffffffffffwb == 9223372036854775807
max
#endif
