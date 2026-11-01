//@ expect: compile-ok
//@ strict-diags
// [meta.unary.prop]/8 with clang's `InitializeTemporary`: the constructed
// object is a temporary, so its destructor must be accessible and not
// deleted, and a potentially-throwing destructor makes the nothrow form
// false (libc++ is_constructible / is_nothrow_default_constructible).
struct PrivateDtor {
    PrivateDtor(int) {}
private:
    ~PrivateDtor() {}
};
struct DeletedDtor { DeletedDtor() = default; ~DeletedDtor() = delete; };
struct DThrows { DThrows() noexcept {} ~DThrows() noexcept(false) {} };
struct Fine { Fine() noexcept {} ~Fine() {} };

static_assert(!__is_constructible(PrivateDtor, int));
static_assert(__is_constructible(PrivateDtor&, PrivateDtor&));
static_assert(!__is_constructible(DeletedDtor));
static_assert(__is_constructible(DThrows));
static_assert(!__is_nothrow_constructible(DThrows));
static_assert(!__is_nothrow_constructible(const DThrows));
static_assert(__is_nothrow_constructible(Fine));
static_assert(__is_nothrow_constructible(DThrows&, DThrows&));

class Friendly {
    ~Friendly() {}
    friend struct Probe;
};
struct Probe { static constexpr bool v = __is_constructible(Friendly); };
static_assert(!__is_constructible(Friendly));
static_assert(!Probe::v);  // traits are checked from an unrelated context
