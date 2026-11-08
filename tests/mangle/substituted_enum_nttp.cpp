// Mangling differential: a class template's enumeration non-type
// parameter, substituted into a still-dependent member template
// signature, mangles as the template argument `L <enum> <value> E`
// (spec/abi.md §8.15, WP-818; [temp.over.link]), not as a cast of the
// replacement literal (clang `SubstNonTypeTemplateParmExprClass`).

enum Policy { single, mutex, atomic };
enum class Scoped : unsigned char { a = 1, b = 7 };
template <class T, Policy P> struct weak {};
template <class T, Scoped S> struct tagged {};
template <class T, Policy P = atomic> struct shared {
  template <class U> bool owner_before(const weak<U, P>&) const { return true; }
  template <class U> shared(const shared<U, P>&) {}
  shared() {}
};
template <class T, Scoped S> struct holder {
  template <class U> void take(tagged<U, S>) {}
};
void use() {
  shared<int> a;
  shared<long> b(a);
  a.owner_before(weak<char, atomic>());
  holder<int, Scoped::b>().take(tagged<char, Scoped::b>());
}
