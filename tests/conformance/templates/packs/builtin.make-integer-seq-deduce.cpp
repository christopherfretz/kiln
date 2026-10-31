// __make_integer_seq: a function template deduces the generated pack.
template <class T, T... Ns> struct seq {};
template <class T, T... Ns> constexpr T total(seq<T, Ns...>) { return (Ns + ... + T(0)); }
static_assert(total(__make_integer_seq<seq, int, 101>{}) == 5050);
