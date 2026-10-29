// [temp.dep.type]: a transform of a dependent type is dependent and is
// applied at instantiation (spec/sema-templates.md §9.2).
//@ strict-diags
enum E : long { e0 };
template <class T> struct Tr {
  typedef __remove_cvref(T) plain;
  typedef __add_pointer(T) ptr;
  typedef __decay(T) decayed;
};
template <class T> struct Sign { typedef __make_unsigned(T) type; };
template <class T> struct U { typedef __underlying_type(T) type; };
static_assert(__is_same(Tr<const int &>::plain, int));
static_assert(__is_same(Tr<int &>::ptr, int *));
static_assert(__is_same(Tr<int[4]>::decayed, int *));
static_assert(__is_same(Sign<const short>::type, const unsigned short));
static_assert(__is_same(U<E>::type, long));
template <class T> using RemoveRef = __remove_reference_t(T);
static_assert(__is_same(RemoveRef<int &&>, int));
