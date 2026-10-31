// A freestanding std::initializer_list ([support.initlist]) with the
// libc++ layout (pointer, size). The compiler fills the two members
// directly from the backing array ([dcl.init.list]/5).
#pragma once
namespace std {
using size_t = decltype(sizeof 0);
template <class E> class initializer_list {
  const E* b;
  size_t n;

public:
  using value_type = E;
  using size_type = size_t;
  constexpr initializer_list() noexcept : b(nullptr), n(0) {}
  constexpr size_t size() const noexcept { return n; }
  constexpr const E* begin() const noexcept { return b; }
  constexpr const E* end() const noexcept { return b + n; }
};
} // namespace std
