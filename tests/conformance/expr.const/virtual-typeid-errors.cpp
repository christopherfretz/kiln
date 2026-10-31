// [expr.typeid]/3, [expr.const]/5, CWG1517: typeid of a null polymorphic
// glvalue (a throw), of an object with an unknown dynamic type, of a
// member outside its lifetime and of an object still constructing its
// bases (clang's notes).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:16 error "constexpr variable 'x1' must be initialized by a constant expression"
//@ diag: 33:49 note "typeid applied to dereferenced null pointer is not allowed in a constant expression"
//@ diag: 34:21 note "in call to 'null()'"
//@ diag: 39:16 error "constexpr variable 'x2' must be initialized by a constant expression"
//@ diag: 38:48 note "typeid applied to object 'ext' whose dynamic type is not constant"
//@ diag: 39:21 note "in call to 'unknown()'"
//@ diag: 49:16 error "constexpr variable 'x3' must be initialized by a constant expression"
//@ diag: 45:40 note "typeid applied to object outside its lifetime is not allowed in a constant expression"
//@ diag: 46:21 note "in call to 'this->is_b()'"
//@ diag: 48:28 note "in call to 'H()'"
//@ diag: 49:21 note "in call to 'early()'"
//@ diag: 60:16 error "constexpr variable 'x4' must be initialized by a constant expression"
//@ diag: 57:21 note "subexpression not valid in a constant expression"
//@ diag: 59:35 note "in call to 'F()'"
//@ diag: 60:21 note "in call to 'in_base_init()'"
namespace std {
class type_info {
public:
  constexpr bool operator==(const type_info &o) const { return this == &o; }
};
} // namespace std

struct A { constexpr virtual ~A() {} };
struct B : A {};

// typeid of a null polymorphic glvalue throws std::bad_typeid.
constexpr bool null() { A *pa = nullptr; return typeid(*pa) == typeid(A); }
constexpr bool x1 = null();

// An object whose dynamic type the evaluation cannot know.
extern B ext;
constexpr bool unknown() { A &ra = ext; return typeid(ra) == typeid(B); }
constexpr bool x2 = unknown();

// A member whose construction has not started is outside its lifetime.
struct H {
  bool t;
  B b;
  constexpr bool is_b() const { return typeid(b) == typeid(B); }
  constexpr H() : t(is_b()) {}
};
constexpr bool early() { H h; return h.t; }
constexpr bool x3 = early();

// A base whose construction has not started yet (CWG1517).
struct E : A {
  bool t;
  constexpr E(bool b) : t(b) {}
};
struct F : E {
  constexpr F() : E(typeid(*this) == typeid(F)) {}
};
constexpr bool in_base_init() { F f; return f.t; }
constexpr bool x4 = in_base_init();
