// [temp.deduct.call]/1: each element is deduced independently.
template <class... Ts> constexpr bool second_is_double(Ts...) {
  return __is_same(__type_pack_element<1, Ts...>, double);
}
static_assert(second_is_double(1, 2.0));
static_assert(!second_is_double(1, 2.0f));
