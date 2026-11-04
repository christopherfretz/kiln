// [cpp.cond]/5 (`__has_cpp_attribute`), [dcl.attr.grammar]/6: clang's
// thread-safety attributes (WP-606; spec/sema.md §4.5) are reported as
// supported: `__has_attribute(x)` is 1 for every member of the family, and
// `__has_cpp_attribute(clang::x)` is 1 for those clang spells `Clang<"x">`
// and 0 for the GNU-only ones. `reentrant_capability` is clang 22's.
//@ expect: compile-ok
//@ strict-diags
#define GNU_KNOWN(x) static_assert(__has_attribute(x) == 1, #x);
#define CLANG_KNOWN(x) static_assert(__has_cpp_attribute(clang::x) == 1, #x);
#define CLANG_UNKNOWN(x) static_assert(__has_cpp_attribute(clang::x) == 0, #x);
#define BOTH(x) GNU_KNOWN(x) CLANG_KNOWN(x)
#define GNU_ONLY(x) GNU_KNOWN(x) CLANG_UNKNOWN(x)

BOTH(capability)
BOTH(shared_capability)
BOTH(scoped_lockable)
BOTH(guarded_var)
BOTH(pt_guarded_var)
BOTH(requires_capability)
BOTH(requires_shared_capability)
BOTH(exclusive_locks_required)
BOTH(shared_locks_required)
BOTH(acquire_capability)
BOTH(acquire_shared_capability)
BOTH(try_acquire_capability)
BOTH(try_acquire_shared_capability)
BOTH(release_capability)
BOTH(release_shared_capability)
BOTH(release_generic_capability)
BOTH(unlock_function)
BOTH(assert_capability)
BOTH(assert_shared_capability)
BOTH(no_thread_safety_analysis)
GNU_ONLY(lockable)
GNU_ONLY(guarded_by)
GNU_ONLY(pt_guarded_by)
GNU_ONLY(acquired_before)
GNU_ONLY(acquired_after)
GNU_ONLY(exclusive_lock_function)
GNU_ONLY(shared_lock_function)
GNU_ONLY(exclusive_trylock_function)
GNU_ONLY(shared_trylock_function)
GNU_ONLY(assert_exclusive_lock)
GNU_ONLY(assert_shared_lock)
GNU_ONLY(locks_excluded)
GNU_ONLY(lock_returned)
#if __clang_major__ >= 22
BOTH(reentrant_capability)
#else
static_assert(!__has_attribute(reentrant_capability));
#endif
// The reserved spellings answer the same.
static_assert(__has_attribute(__guarded_by__) == 1);
static_assert(__has_cpp_attribute(_Clang::capability) == 1);
// A near miss is not a member.
static_assert(!__has_attribute(guarded_by_mutex));
static_assert(!__has_cpp_attribute(gnu::guarded_by));
