// [class.access.general]/2 and /7, [class.friend]/2, [temp.deduct.general]/8:
// the access context of a function template's signature substitution ends
// with it. Substituting explicit template arguments into a template declared
// in a local scope (a generic lambda's `operator()<I>`) left that template as the access context for the rest of the
// translation unit, so a class's own member function bodies, its defaulted
// comparison and a befriended specialization's static data member
// initializer analysed later lost access to the class's private members.
// Reduced from real code (WP-1726, a regression since 1.2.1).
//@ expect: compile-ok
//@ std: c++20

namespace lib {
template <class T> struct meta;
template <class... M> constexpr int object(M...) { return sizeof...(M); }
template <int N, class F> constexpr void for_each(F &&f) {
  [&]<int... I>(int (*)[sizeof...(I) + 1]) {
    (f.template operator()<I>(), ...);
  }.template operator()<0, 1, 2>(nullptr);
  (void)N;
}
} // namespace lib

auto suite_a = [] {
  int n = 0;
  []<int I>(int &m) { m += I; }.operator()<2>(n);
  lib::for_each<3>([&]<int I>() { n += I; });
  return n;
}();

class own_member {
  int value_{7};

public:
  own_member() = default;
  int value() const { return value_; }
};

struct put_action {
  int data() const { return m_data; }
  bool operator==(const put_action &) const = default;

private:
  int m_data{};
};

class private_fields {
  double cash = 22.0;
  int currency = 1;
  friend struct lib::meta<private_fields>;
};

template <> struct lib::meta<private_fields> {
  using T = private_fields;
  static constexpr auto value = object(&T::cash, &T::currency);
};

static_assert(lib::meta<private_fields>::value == 2);

int main() {
  own_member o;
  put_action a, b;
  return o.value() == 7 && a == b && a.data() == 0 ? 0 : 1;
}
