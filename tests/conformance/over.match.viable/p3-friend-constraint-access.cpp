// [over.match.viable]/3 with [class.friend]/2: a non-template friend's
// trailing requires-clause is checked from the friend, so it may name a private
// member of the befriending class; the friend stays viable and, as a
// non-template, beats an equally good template ([over.match.best.general]/2.4).
// Reduced from real code: an iterator adaptor whose
// constrained `operator-` kiln found unsatisfied when the class was in a
// namespace, so the deduced-return template was chosen and recursed.
//@ expect: run
//@ stdout: 2
#include <cstdio>
#include <iterator>

namespace lib {
template <class Base>
class Conv {
    Base it_;

  public:
    using difference_type = std::ptrdiff_t;
    Conv() = default;
    Conv(Base it) : it_(it) {}
    bool operator==(std::sentinel_for<Base> auto rhs) const { return it_ == rhs; }
    friend bool operator==(const Conv &l, const Conv &r) { return l.it_ == r.it_; }
    friend auto operator-(std::sentinel_for<Base> auto s, Conv i)
        requires std::random_access_iterator<Base>
    {
        return s - i.it_;
    }
    friend difference_type operator-(const Conv &l, const Conv &r)
        requires requires { l.it_ - r.it_; }
    {
        return l.it_ - r.it_;
    }
};
} // namespace lib

int main() {
    const char *a[2] = {"x", "y"};
    lib::Conv<const char *const *> f(a), l(a + 2);
    std::printf("%d\n", int(l - f));
}
