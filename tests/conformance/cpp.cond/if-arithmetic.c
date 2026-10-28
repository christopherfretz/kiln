// [cpp.cond] (C mode): #if arithmetic uses intmax_t/uintmax_t with the usual
// arithmetic conversions, and && / || short-circuit evaluation.
//@ expect: pp
//@ std: c11
//@ oracle: clang
#if -1 < 0u
signed_vs_unsigned_wrong
#else
signed_vs_unsigned_converted
#endif
#if 1 || (1 / 0)
short_circuit
#endif
#if 0x7fffffffffffffff > 0 && defined(__STDC__)
intmax_ok
#endif
