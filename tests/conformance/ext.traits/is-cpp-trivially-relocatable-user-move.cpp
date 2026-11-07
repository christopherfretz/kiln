// __builtin_is_cpp_trivially_relocatable(T), [class.prop] default-movable:
// a user-provided move operation disqualifies a class even when the
// matching copy operation is implicit (a user-declared move deletes the
// implicit copy, and an xvalue selects the move). abseil's
// TriviallyRelocatable.UserProvidedMoveAssignment test. Oracle-checked by
// hand with clang++-22 (22.1.8): same answers.
//@ requires: clang>=22
//@ expect: compile-ok
struct MoveAssign { MoveAssign(MoveAssign&&) = default; MoveAssign& operator=(MoveAssign&&) { return *this; } };
struct MoveCtor { MoveCtor(MoveCtor&&) {} MoveCtor& operator=(MoveCtor&&) = default; };
struct OnlyMoveAssign { OnlyMoveAssign& operator=(OnlyMoveAssign&&) { return *this; } };
struct CopyAssign { CopyAssign(const CopyAssign&) = default; CopyAssign& operator=(const CopyAssign&) { return *this; } };
struct Defaulted { Defaulted(Defaulted&&) = default; Defaulted& operator=(Defaulted&&) = default; };
struct Plain { int x; };
static_assert(!__builtin_is_cpp_trivially_relocatable(MoveAssign), "");
static_assert(!__builtin_is_cpp_trivially_relocatable(MoveCtor), "");
static_assert(!__builtin_is_cpp_trivially_relocatable(OnlyMoveAssign), "");
static_assert(!__builtin_is_cpp_trivially_relocatable(CopyAssign), "");
static_assert(__builtin_is_cpp_trivially_relocatable(Defaulted), "");
static_assert(__builtin_is_cpp_trivially_relocatable(Plain), "");
static_assert(__builtin_is_cpp_trivially_relocatable(MoveAssign[2]) == false, "");
int main() {}
