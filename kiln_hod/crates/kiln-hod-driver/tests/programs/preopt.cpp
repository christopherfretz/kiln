// A pre-opt module (WP-1562): `always_inline` functions, an
// `always_inline` call site and a coroutine, which need `opt` at -O0 on
// the LLVM route and which hod compiles itself.
#include <coroutine>
#include <cstdio>
#include <vector>

[[gnu::always_inline]] inline int twice(int x) { return 2 * x; }
int plus_one(int x) { return x + 1; }

struct gen {
    struct promise_type {
        int value = 0;
        gen get_return_object() { return gen{std::coroutine_handle<promise_type>::from_promise(*this)}; }
        std::suspend_always initial_suspend() noexcept { return {}; }
        std::suspend_always final_suspend() noexcept { return {}; }
        std::suspend_always yield_value(int v) noexcept {
            value = v;
            return {};
        }
        void return_void() {}
        void unhandled_exception() {}
    };
    std::coroutine_handle<promise_type> h;
    ~gen() { h.destroy(); }
    bool next() {
        h.resume();
        return !h.done();
    }
};

gen count(int n) {
    for (int i = 0; i < n; ++i) co_yield twice(i);
}

int main() {
    std::vector<int> v;
    gen g = count(4);
    while (g.next()) v.push_back(g.h.promise().value);
    int sum = 0;
    for (int x : v) {
        [[clang::always_inline]] sum += plus_one(x);
    }
    std::printf("yielded %zu sum %d\n", v.size(), sum);
    return 0;
}
