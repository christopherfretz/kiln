// [dcl.constexpr]/6, [stmt.dcl]/2, [basic.def.odr]/4: a block-scope
// `constexpr` variable is an automatic object like any other. At -O0 it
// gets its `alloca` and its initialization (a `memcpy` from a private
// `@__const.<fn>.<var>`, a `memset` when its value is all zero, a store
// for a scalar) whether or not anything reads it at run time: clang 19/22
// and GCC 16.1 all keep the storage of a local that only consteval calls
// read (spec/codegen.md "Constexpr locals"). Shapes reduced from real
// code: only passed by reference to a consteval function, in a function
// template instance, read by several consteval calls inside a designated
// initializer, used in a discarded statement only; and the shapes that use
// the object at run time: address taken, bound to a reference that outlives
// the full-expression, passed by reference to a non-consteval function,
// captured by reference in a lambda that runs. The clang 19 IR (the
// sibling without `-19` pins clang 22's). WP-1410.
//@ requires: clang<22
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
struct index_t { bool a; bool b; int t[4]; };
struct zero_t { int a; int b[3]; };
struct out { int x; int y; bool z; };

consteval index_t make() { index_t r{}; r.b = true; r.t[2] = 3; return r; }
consteval bool pick(const index_t& i) { return i.b; }
consteval int third(const index_t& i) { return i.t[2]; }
consteval int za(const zero_t& z) { return z.a; }
int observe(const index_t& i) { return i.t[2]; }

int only_consteval() {
    constexpr index_t idx = make();
    const bool b = pick(idx);
    return b ? 1 : 0;
}

int all_zero() {
    constexpr zero_t z{};
    return za(z);
}

template <class T> out designated() {
    constexpr index_t idx = make();
    return out{.x = third(idx), .y = pick(idx) ? 2 : 0, .z = pick(idx)};
}

template <class T> int discarded() {
    constexpr index_t idx = make();
    if constexpr (sizeof(T) == 0) {
        return observe(idx);
    }
    return 0;
}

int address() {
    constexpr index_t idx = make();
    const index_t* p = &idx;
    return observe(*p);
}

int bound() {
    constexpr index_t idx = make();
    const index_t& r = idx;
    return r.t[2];
}

int by_ref() {
    constexpr index_t idx = make();
    return observe(idx);
}

int captured() {
    constexpr index_t idx = make();
    auto f = [&] { return observe(idx); };
    return f();
}

int main() {
    return only_consteval() + all_zero() + designated<int>().x + discarded<int>() + address()
        + bound() + by_ref() + captured() - 16;
}
