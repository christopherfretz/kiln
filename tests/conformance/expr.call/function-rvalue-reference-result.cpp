//@ expect: run
//@ strict-diags
// [expr.call]/14: a call whose result type is an rvalue reference to a
// function is an lvalue, so it can be called and decltype gives F&
// (libc++ is_nothrow_invocable_r<CantMove, Fn> through declval<Fn>()()).
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;
template <class T> T&& declval() noexcept;

int seven() { return 7; }
int (&&get())() { return static_cast<int (&&)()>(seven); }
using Fn = int() noexcept;
static_assert(same<decltype(get()), int (&)()>);
static_assert(same<decltype(declval<Fn>()), Fn&>);
static_assert(same<decltype(declval<Fn>()()), int>);

struct CantMove {
    CantMove() = default;
    CantMove(CantMove&&) = delete;
};
template <class T> void sink(T) noexcept;
static_assert(noexcept(sink<CantMove>(declval<CantMove() noexcept>()())));

int main() {
    int (&r)() = get();
    return get()() + r() == 14 ? 0 : 1;
}
