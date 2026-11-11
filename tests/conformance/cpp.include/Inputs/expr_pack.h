#ifndef EXPR_PACK_H
#define EXPR_PACK_H
template <class... Ts> struct pk_list { static constexpr unsigned long n = sizeof...(Ts); };
template <class... Ts> using pk_twice = pk_list<Ts..., Ts...>;
template <class... Ts> using pk_ptrs = pk_list<Ts *...>;
template <class... Ts> pk_list<Ts..., int> pk_append(Ts...) { return {}; }
#endif
