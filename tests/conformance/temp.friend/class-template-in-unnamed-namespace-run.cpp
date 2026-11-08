// [temp.friend]/1, [namespace.memdef]/3, [namespace.unnamed]/1: `template <class C> friend
// class F;` in a class of namespace `t` names the class template `F` that ordinary lookup finds
// in an unnamed namespace nested in `t` (through its implicit using-directive), so that template
// is befriended and no new one is declared in `t`. The same holds for an inline namespace
// (gtest's `ParameterizedTestFactory` and `WithParamInterface::SetParam`; WP-803).
//@ expect: run
//@ strict-diags
//@ stdout: 4 6 1
#include <cstdio>

namespace t {
namespace {
template <class C> struct F {
    int run() { return C::get(); }
};
} // namespace
template <class T> class W {
    static int get() { return 4; }
    template <class C> friend class F;
};
inline namespace v1 {
template <class C> struct H {
    int run() { return C::get3(); }
};
} // namespace v1
class X {
    static int get3() { return 6; }
    template <class C> friend struct H;
};
} // namespace t

int main() {
    // One template `F`: the friend did not declare `t::F` beside `t::(anonymous)::F`.
    std::printf("%d %d %d\n", t::F<t::W<int>>().run(), t::H<t::X>().run(),
                int(sizeof(t::F<int>) == sizeof(t::F<long>)));
}
