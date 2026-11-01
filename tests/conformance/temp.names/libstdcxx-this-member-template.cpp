// [temp.names]/3: in `this->emplace<I>(...)` the name is looked up in the
// class of the object expression (the current instantiation) and found to be
// a member template, so `<` starts a template argument list without
// `template`. libstdc++ variant:1526 (`this->emplace<__index>(...)`);
// 27 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): member template of the current instantiation after this-> not treated as a template
template<class... Ts> struct variant {
  template<unsigned I, class... A> int emplace(A &&...) { return int(I); }
  template<class T> static constexpr unsigned index_of = 1;
  template<class T, class... A> int emplace(A &&...a) {
    constexpr unsigned index = index_of<T>;
    return this->emplace<index>(static_cast<A &&>(a)...);
  }
};
int g() { return variant<int, long>().emplace<long>(1); }
