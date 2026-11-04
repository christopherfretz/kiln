// [temp.dep.type]/1: a typedef naming the current instantiation
// (`typedef pb<B> this_type;`) qualifies a member of it as `pb<B>::` does,
// so `typename this_type::pos_type` in the class is the member typedef and
// the out-of-line definitions' `typename pb<B>::pos_type` matches it
// (boost's `basic_pointerbuf::seekoff`).
//@ strict-diags
template <class B> struct pb : B {
  typedef pb<B> this_type;
  typedef typename B::pos_type pos_type;
  typename this_type::pos_type seekpos();
  typename pb::pos_type seekpos2();
  typename pb<B>::pos_type seekpos3();
  pb() { this_type::reset(); }
};
template <class B> typename pb<B>::pos_type pb<B>::seekpos() { return 0; }
template <class B> typename pb<B>::pos_type pb<B>::seekpos2() { return 0; }
template <class B> typename pb<B>::pos_type pb<B>::seekpos3() { return 0; }
struct base { typedef long pos_type; void reset() {} };
long f() { pb<base> p; return p.seekpos() + p.seekpos2() + p.seekpos3(); }
