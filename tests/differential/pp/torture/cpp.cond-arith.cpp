//@ expect: pp
//@ args: --target=aarch64-unknown-linux-gnu
// The target is pinned because '\377' < 0 depends on char signedness
// (unsigned on aarch64, signed on x86_64); the golden is the aarch64 answer.
// #if arithmetic: intmax_t/uintmax_t promotion, unsigned wraparound,
// shifts, the conditional operator, character literals, identifiers that
// are not macros (0), true/false in C++, short-circuiting of division by
// zero, and the comma operator in an unevaluated operand.
#if -1 < 0u
wrong_signed_compare;
#else
unsigned_compare;
#endif
#if 0xFFFFFFFFFFFFFFFF == -1
all_ones_equal;
#endif
#if (1 ? -1 : 0u) > 0
conditional_converts;
#endif
#if 'A' == 65 && '\n' == 10 && '\377' < 0
char_signed;
#elif 'A' == 65
char_unsigned_or_other;
#endif
#if not_a_macro == 0 && !undefined_too
identifiers_are_zero;
#endif
#if true && !false
cxx_booleans;
#endif
#if 0 && (1 / 0)
#else
short_circuit;
#endif
#if 1 || (1 % 0)
short_circuit_or;
#endif
#if (2 || 3) == 1 && (0 ? 1/0 : 2) == 2
logical_results;
#endif
#if (1 << 63) < 0 && (1u << 63) > 0 && (-8 >> 1) == -4
shifts;
#endif
#if 10 / 3 == 3 && -7 / 2 == -3 && -7 % 2 == -1 && 1'000 == 1000
division;
#endif
#if 0b101 == 5 && 017 == 15 && 0xfULL == 15 && 18446744073709551615u == -1
literal_forms;
#endif
