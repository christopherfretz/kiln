// [meta.unary.prop]: class property predicates, positive and negative
// (spec/sema-templates.md §9.1).
//@ strict-diags
struct Empty {};
struct NonEmpty { int x; };
struct Poly { virtual void f(); };
struct Abstract { virtual void f() = 0; };
struct Final final {};
struct VDtor { virtual ~VDtor(); };
struct Derived : Poly {};
struct Agg { int a; double b; };
struct DerivedAgg : Agg { int c; };
struct WithCtor { WithCtor(int); int a; };
struct Mixed { int a; private: int b; };
struct NonTrivialCopy { NonTrivialCopy(const NonTrivialCopy &); };
struct Padded { char c; int i; };
union U { int i; };

static_assert(__is_empty(Empty) && __is_empty(const Empty));
static_assert(!__is_empty(NonEmpty) && !__is_empty(Poly) && !__is_empty(int) && !__is_empty(U));
static_assert(__is_polymorphic(Poly) && __is_polymorphic(Derived) && __is_polymorphic(Abstract));
static_assert(!__is_polymorphic(Empty) && !__is_polymorphic(int));
static_assert(__is_abstract(Abstract) && !__is_abstract(Poly) && !__is_abstract(int));
static_assert(__is_final(Final) && !__is_final(Empty) && !__is_final(int));
static_assert(__is_aggregate(Agg) && __is_aggregate(int[2]) && __is_aggregate(Empty) && __is_aggregate(DerivedAgg));
static_assert(!__is_aggregate(WithCtor) && !__is_aggregate(int) && !__is_aggregate(Poly) && !__is_aggregate(Mixed) && !__is_aggregate(Derived));
static_assert(__has_virtual_destructor(VDtor) && !__has_virtual_destructor(Poly) && !__has_virtual_destructor(int));
static_assert(__is_standard_layout(Agg) && __is_standard_layout(int) && __is_standard_layout(Empty));
static_assert(!__is_standard_layout(Mixed) && !__is_standard_layout(Poly) && !__is_standard_layout(int &));
static_assert(__is_trivial(Agg) && __is_trivial(int) && __is_trivial(Mixed));
static_assert(!__is_trivial(WithCtor) && !__is_trivial(Poly) && !__is_trivial(NonTrivialCopy));
static_assert(__is_pod(Agg) && __is_pod(int) && __is_pod(int[2]));
static_assert(!__is_pod(Mixed) && !__is_pod(Poly) && !__is_pod(WithCtor));
static_assert(__is_literal_type(int) && __is_literal_type(Agg) && __is_literal_type(int &));
static_assert(!__is_literal_type(VDtor) && !__is_literal_type(void()));
static_assert(__is_trivially_copyable(Agg) && __is_trivially_copyable(int) && __is_trivially_copyable(WithCtor));
static_assert(!__is_trivially_copyable(NonTrivialCopy) && !__is_trivially_copyable(Poly) && !__is_trivially_copyable(int &));
static_assert(__has_unique_object_representations(int) && __has_unique_object_representations(NonEmpty));
static_assert(!__has_unique_object_representations(float) && !__has_unique_object_representations(Padded));
static_assert(!__has_unique_object_representations(Empty) && !__has_unique_object_representations(int &));
