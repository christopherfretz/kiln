// [temp.spec.partial.match]: a non-type pack in a partial specialization.
template <class T, T... Ns> struct seq {};
template <class S> struct ssum;
template <int... Ns> struct ssum<seq<int, Ns...>> { static constexpr int v = (Ns + ... + 0); };
static_assert(ssum<seq<int, 1, 2, 3>>::v == 6);
static_assert(ssum<seq<int>>::v == 0);
