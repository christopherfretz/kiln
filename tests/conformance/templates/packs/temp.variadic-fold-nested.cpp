// [temp.variadic]: sizeof... inside a fold pattern is not itself expanded.
template <class... Ts> constexpr unsigned f() { return (sizeof...(Ts) + ... + sizeof(Ts)); }
static_assert(f<char, char>() == 4);
static_assert(f<>() == 0);
