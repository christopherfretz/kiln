// [expr.dynamic.cast]/9, [expr.typeid]/3: a failed reference cast throws
// std::bad_cast and typeid of a null glvalue throws std::bad_typeid;
// both are invoked under cleanups.
namespace std { class type_info; }
struct B { virtual ~B(); };
struct D : B {};
struct S { S(); ~S(); };
D& dc(B& b) { S s; return dynamic_cast<D&>(b); }
const std::type_info& ti(B* b) { S s; return typeid(*b); }
