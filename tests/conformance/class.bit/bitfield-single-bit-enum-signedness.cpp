// [class.bit]/1, [conv.integral]/3: -Wsingle-bit-bitfield-constant-conversion
// reports a 1 stored into a one-bit bit-field whose type reads back signed,
// clang's `isSignedIntegerType()` of the bit-field's type. That is false for
// a scoped enumeration whatever its underlying type ([dcl.enum]/5: it is not
// an integer type), so `enum class E : int` truncates as unsigned and 1 stays
// 1 (a downstream record header's `Av a : 1; a = Av::Has;`, a
// -Werror failure in 10 TUs). An unscoped enumeration follows its underlying
// type: `int` (fixed, or chosen for a negative enumerator) and `signed char`
// warn, `unsigned int` (chosen without one) and `unsigned char` do not. A
// value that does not fit is still -Wbitfield-constant-conversion for every
// enumeration; a scoped one's -1 reads back as 1. `bool` takes any value.
//@ args: -Wall
//@ strict-diags
//@ diag: 37:8 warning "implicit truncation from 'SE' to a one-bit wide bit-field changes value from 1 to -1"
//@ diag: 38:7 warning "implicit truncation from 'FE' to a one-bit wide bit-field changes value from 1 to -1"
//@ diag: 41:7 warning "implicit truncation from 'int' to a one-bit wide bit-field changes value from 1 to -1"
//@ diag: 43:8 warning "implicit truncation from 'Av' to bit-field changes value from 2 to 0"
//@ diag: 44:7 warning "implicit truncation from 'Sv' to bit-field changes value from 2 to 0"
//@ diag: 45:7 warning "implicit truncation from 'Sv' to bit-field changes value from -1 to 1"
//@ diag: 46:7 warning "implicit truncation from 'SE' to bit-field changes value from 2 to -2"
//@ diag: 50:33 warning "implicit truncation from 'SE' to a one-bit wide bit-field changes value from 1 to -1"
//@ diag: 50:37 warning "implicit truncation from 'FE' to a one-bit wide bit-field changes value from 1 to -1"
//@ diag: 50:51 warning "implicit truncation from 'int' to a one-bit wide bit-field changes value from 1 to -1"
enum class Av { Waiting, Has, Other };
enum class Sv : int { A, B, C };
enum UE { UA, UB, UC };
enum SE { SA = -1, SB = 1 };
enum FE : signed char { FA, FB };
enum FU : unsigned char { GA, GB };
struct R {
  Av a : 1; Sv s : 1; UE u : 1; SE e : 2; SE e1 : 1; FE f : 1; FU g : 1;
  bool b : 1; int i : 1; unsigned ui : 1; Av a2 : 1; Sv s2 : 2;
  void set() {
    a = Av::Has;
    s = Sv::B;
    u = UB;
    e1 = SB;
    f = FB;
    g = GB;
    b = true;
    i = 1;
    ui = 1;
    a2 = Av::Other;
    s = Sv::C;
    s = Sv(-1);
    e = SE(2);
    s2 = Sv::C;
  }
};
R r2 = {Av::Has, Sv::B, UB, SB, SB, FB, GB, true, 1, 1, Av::Has, Sv::C};
