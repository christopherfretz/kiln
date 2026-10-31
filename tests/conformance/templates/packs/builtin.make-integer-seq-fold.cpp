// __make_integer_seq produces a pack consumed by a variadic partial specialization.
template <class T, T... Ns> struct seq {};
template <class S> struct ssum;
template <class T, T... Ns> struct ssum<seq<T, Ns...>> { static constexpr T v = (Ns + ... + 0); };
static_assert(ssum<__make_integer_seq<seq, int, 5>>::v == 10);
static_assert(ssum<__make_integer_seq<seq, long, 0>>::v == 0);
