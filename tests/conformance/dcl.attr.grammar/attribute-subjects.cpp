// [dcl.attr.grammar]/5 (an attribute appertains to the entity its position names; ill-formed
// when it cannot), [dcl.attr.nodiscard]/1, [dcl.attr.noreturn]/1, [dcl.attr.nouniqueaddr]/1,
// [dcl.align]/1, [dcl.attr.fallthrough]/1, [dcl.attr.likelihood]/1, [dcl.attr.assume]/1,
// [dcl.typedef]/2 (an alias-declaration's attributes appertain to the typedef-name),
// [dcl.spec.general]/1, [dcl.ptr]/1, [dcl.fct]/1 (attributes after the decl-specifier-seq, a
// ptr-operator or a function declarator appertain to the type): one subject table for
// declarators, alias-declarations, typedefs, class-heads and enum-heads, and clang 19's "cannot
// be applied to types" for declaration attributes in a type position. Pinned to C++23 (P3475R2
// removes carries_dependency in C++26: dcl.attr.depend/carries-dependency-removed-cxx26.cpp).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 182:12 warning "'[[nodiscard]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 183:12 error "'noreturn' attribute only applies to functions"
//@ diag: 185:10 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 186:12 error "'fallthrough' attribute cannot be applied to a declaration"
//@ diag: 187:12 error "'no_unique_address' attribute only applies to non-bit-field non-static data members"
//@ diag: 188:12 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 189:26 warning "'packed' attribute ignored [-Wignored-attributes]"
//@ diag: 190:13 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 193:13 warning "'[[nodiscard]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 194:19 error "'deprecated' attribute cannot be applied to types"
//@ diag: 195:17 error "'alignas' attribute cannot be applied to types"
//@ diag: 196:21 error "'maybe_unused' attribute cannot be applied to types"
//@ diag: 198:13 error "'unlikely' attribute cannot be applied to a declaration"
//@ diag: 199:13 error "'assume' attribute cannot be applied to a declaration"
//@ diag: 200:39 error "'noreturn' attribute only applies to functions"
//@ diag: 201:13 warning "unknown attribute 'foo' ignored [-Wunknown-attributes]"
//@ diag: 202:26 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
//@ diag: 204:18 error "'noreturn' attribute only applies to functions"
//@ diag: 205:16 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 206:1 warning "'A1' is deprecated [-Wdeprecated-declarations]"
//@ diag: 180:12 note "'A1' has been explicitly marked deprecated here"
//@ diag: 207:1 warning "'A13' is deprecated: m [-Wdeprecated-declarations]"
//@ diag: 192:13 note "'A13' has been explicitly marked deprecated here"
//@ diag: 208:1 warning "'A25' is deprecated [-Wdeprecated-declarations]"
//@ diag: 203:13 note "'A25' has been explicitly marked deprecated here"
//@ diag: 211:19 warning "'[[nodiscard]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 212:19 error "'noreturn' attribute only applies to functions"
//@ diag: 214:17 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 215:19 error "'fallthrough' attribute cannot be applied to a declaration"
//@ diag: 216:19 error "'no_unique_address' attribute only applies to non-bit-field non-static data members"
//@ diag: 217:19 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 218:33 warning "'packed' attribute ignored [-Wignored-attributes]"
//@ diag: 219:20 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 222:20 warning "'[[nodiscard]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 224:20 error "'unlikely' attribute cannot be applied to a declaration"
//@ diag: 225:20 error "'assume' attribute cannot be applied to a declaration"
//@ diag: 226:46 error "'noreturn' attribute only applies to functions"
//@ diag: 227:20 warning "unknown attribute 'foo' ignored [-Wunknown-attributes]"
//@ diag: 228:33 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
//@ diag: 231:34 error "an attribute list cannot appear here"
//@ diag: 231:67 error "an attribute list cannot appear here"
//@ diag: 231:105 error "an attribute list cannot appear here"
//@ diag: 232:3 warning "attribute 'nodiscard' cannot be applied to functions without return value [-Wignored-attributes]"
//@ diag: 233:33 warning "attribute 'nodiscard' cannot be applied to functions without return value [-Wignored-attributes]"
//@ diag: 234:3 warning "'nodiscard' attribute only applies to functions, classes, or enumerations [-Wignored-attributes]"
//@ diag: 235:3 warning "attribute 'nodiscard' cannot be applied to functions without return value [-Wignored-attributes]"
//@ diag: 237:12 warning "'[[nodiscard]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 238:12 warning "attribute 'nodiscard' cannot be applied to functions without return value [-Wignored-attributes]"
//@ diag: 244:25 warning "'noreturn' only applies to function types; type here is 'int &' [-Wignored-attributes]"
//@ diag: 245:102 error "'no_unique_address' attribute only applies to non-bit-field non-static data members"
//@ diag: 247:28 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 248:31 warning "'[[nodiscard]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 249:9 warning "warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 249:37 error "'noreturn' attribute only applies to functions"
//@ diag: 249:53 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 250:10 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 251:13 error "'nodiscard' attribute cannot be applied to types"
//@ diag: 252:21 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 253:21 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 254:18 warning "'[[nodiscard]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 255:12 warning "'[[nodiscard]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 256:26 error "'nodiscard' attribute cannot be applied to types"
//@ diag: 257:13 warning "attribute 'nodiscard' cannot be applied to functions without return value [-Wignored-attributes]"
//@ diag: 260:30 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 261:11 error "'noreturn' attribute only applies to functions"
//@ diag: 262:14 error "'noreturn' attribute cannot be applied to types"
//@ diag: 263:22 error "'noreturn' attribute only applies to functions"
//@ diag: 264:22 error "'noreturn' attribute only applies to functions"
//@ diag: 265:19 error "'noreturn' attribute only applies to functions"
//@ diag: 266:13 error "'noreturn' attribute only applies to functions"
//@ diag: 267:27 error "'noreturn' attribute cannot be applied to types"
//@ diag: 268:14 error "'noreturn' attribute only applies to functions"
//@ diag: 269:8 error "'noreturn' attribute only applies to functions"
//@ diag: 270:10 error "'noreturn' attribute only applies to functions"
//@ diag: 271:30 error "'noreturn' attribute only applies to functions"
//@ diag: 272:24 warning "'packed' attribute ignored [-Wignored-attributes]"
//@ diag: 273:27 warning "'packed' attribute ignored [-Wignored-attributes]"
//@ diag: 275:32 warning "'packed' attribute ignored [-Wignored-attributes]"
//@ diag: 276:26 warning "'packed' attribute ignored [-Wignored-attributes]"
//@ diag: 277:40 warning "'packed' attribute ignored [-Wignored-attributes]"
//@ diag: 278:27 warning "'packed' attribute ignored [-Wignored-attributes]"
//@ diag: 281:43 warning "'packed' attribute ignored [-Wignored-attributes]"
//@ diag: 282:11 error "'no_unique_address' attribute only applies to non-bit-field non-static data members"
//@ diag: 283:14 error "'no_unique_address' attribute cannot be applied to types"
//@ diag: 285:22 error "'no_unique_address' attribute only applies to non-bit-field non-static data members"
//@ diag: 286:19 error "'no_unique_address' attribute only applies to non-bit-field non-static data members"
//@ diag: 287:13 error "'no_unique_address' attribute only applies to non-bit-field non-static data members"
//@ diag: 288:27 error "'no_unique_address' attribute cannot be applied to types"
//@ diag: 289:14 error "'no_unique_address' attribute only applies to non-bit-field non-static data members"
//@ diag: 290:8 error "'no_unique_address' attribute only applies to non-bit-field non-static data members"
//@ diag: 291:10 error "'no_unique_address' attribute only applies to non-bit-field non-static data members"
//@ diag: 292:30 error "'no_unique_address' attribute only applies to non-bit-field non-static data members"
//@ diag: 293:11 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 294:14 error "'carries_dependency' attribute cannot be applied to types"
//@ diag: 295:22 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 296:22 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 297:19 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 298:13 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 299:27 error "'carries_dependency' attribute cannot be applied to types"
//@ diag: 300:14 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 301:8 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 302:10 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 303:30 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 304:11 error "'fallthrough' attribute cannot be applied to a declaration"
//@ diag: 305:14 error "'fallthrough' attribute cannot be applied to types"
//@ diag: 306:22 error "'fallthrough' attribute cannot be applied to a declaration"
//@ diag: 307:22 error "'fallthrough' attribute cannot be applied to a declaration"
//@ diag: 308:19 error "'fallthrough' attribute cannot be applied to a declaration"
//@ diag: 309:13 error "'fallthrough' attribute cannot be applied to a declaration"
//@ diag: 310:27 error "'fallthrough' attribute cannot be applied to types"
//@ diag: 311:14 error "'fallthrough' attribute cannot be applied to a declaration"
//@ diag: 312:8 error "'fallthrough' attribute cannot be applied to a declaration"
//@ diag: 313:10 error "'fallthrough' attribute cannot be applied to a declaration"
//@ diag: 314:30 error "'fallthrough' attribute cannot be applied to a declaration"
//@ diag: 315:11 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 316:14 error "'likely' attribute cannot be applied to types"
//@ diag: 317:22 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 318:22 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 319:19 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 320:13 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 321:27 error "'likely' attribute cannot be applied to types"
//@ diag: 322:14 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 323:8 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 324:10 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 325:30 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 326:11 warning "warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 327:14 warning "warn_unused_result' ignored, because it cannot be applied to a type [-Wignored-attributes]"
//@ diag: 328:22 warning "warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 329:22 warning "warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 330:19 warning "'[[gnu::warn_unused_result]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 331:13 warning "'[[gnu::warn_unused_result]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 332:27 warning "warn_unused_result' ignored, because it cannot be applied to a type [-Wignored-attributes]"
//@ diag: 333:15 warning "warn_unused_result' cannot be applied to functions without return value [-Wignored-attributes]"
//@ diag: 336:31 warning "warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 337:25 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
//@ diag: 339:36 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
//@ diag: 340:33 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
//@ diag: 341:27 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
//@ diag: 344:21 warning "'noreturn' attribute only applies to functions and methods [-Wignored-attributes]"
//@ diag: 345:23 warning "'noreturn' attribute only applies to functions and methods [-Wignored-attributes]"
//@ diag: 346:44 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
//@ diag: 348:15 error "'maybe_unused' attribute cannot be applied to types"
//@ diag: 353:28 error "'maybe_unused' attribute cannot be applied to types"
//@ diag: 359:15 error "'deprecated' attribute cannot be applied to types"
//@ diag: 364:28 error "'deprecated' attribute cannot be applied to types"
//@ diag: 379:12 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 380:15 error "'nodiscard' attribute cannot be applied to types"
//@ diag: 381:23 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 382:23 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 383:20 warning "'[[nodiscard]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 384:14 warning "'[[nodiscard]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 385:28 error "'nodiscard' attribute cannot be applied to types"
//@ diag: 386:15 warning "attribute 'nodiscard' cannot be applied to functions without return value [-Wignored-attributes]"
//@ diag: 389:31 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 390:12 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
//@ diag: 391:15 warning "noreturn' ignored, because it cannot be applied to a type [-Wignored-attributes]"
//@ diag: 392:23 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
//@ diag: 393:23 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
//@ diag: 394:20 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
//@ diag: 395:14 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
//@ diag: 396:28 warning "noreturn' ignored, because it cannot be applied to a type [-Wignored-attributes]"
//@ diag: 397:8 warning "noreturn' attribute only applies to functions and methods [-Wignored-attributes]"
//@ diag: 398:10 warning "noreturn' attribute only applies to functions and methods [-Wignored-attributes]"
//@ diag: 399:31 warning "'noreturn' only applies to function types; type here is 'int' [-Wignored-attributes]"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
using A1 [[deprecated]] = int;
using A2 [[maybe_unused]] = int;
using A3 [[nodiscard]] = int;
using A4 [[noreturn]] = int;
using A5 __attribute__((aligned(8))) = int;
using A6 alignas(8) = int;
using A7 [[fallthrough]] = int;
using A8 [[no_unique_address]] = int;
using A9 [[carries_dependency]] = int;
using A10 __attribute__((packed)) = int;
using A11 [[likely]] = int;
using A12 __attribute__((unused)) = int;
using A13 [[deprecated("m")]] = int;
using A14 [[nodiscard("r")]] = int;
using A15 = int [[deprecated]];
using A16 = int alignas(8);
using A17 = int * [[maybe_unused]];
using A18 [[gnu::aligned(16)]] = int;
using A19 [[unlikely]] = int;
using A20 [[assume(true)]] = int;
using A21 [[deprecated, maybe_unused, noreturn]] = int;
using A22 [[foo]] = int;
using A23 __attribute__((noreturn)) = int;
using A25 [[gnu::deprecated]] = int;
typedef int T1 [[noreturn]];
typedef int T2 alignas(8);
A1 a1;
A13 a13;
A25 a25;
typedef int TA1 [[deprecated]];
typedef int TA2 [[maybe_unused]];
typedef int TA3 [[nodiscard]];
typedef int TA4 [[noreturn]];
typedef int TA5 __attribute__((aligned(8)));
typedef int TA6 alignas(8);
typedef int TA7 [[fallthrough]];
typedef int TA8 [[no_unique_address]];
typedef int TA9 [[carries_dependency]];
typedef int TA10 __attribute__((packed));
typedef int TA11 [[likely]];
typedef int TA12 __attribute__((unused));
typedef int TA13 [[deprecated("m")]];
typedef int TA14 [[nodiscard("r")]];
typedef int TA18 [[gnu::aligned(16)]];
typedef int TA19 [[unlikely]];
typedef int TA20 [[assume(true)]];
typedef int TA21 [[deprecated, maybe_unused, noreturn]];
typedef int TA22 [[foo]];
typedef int TA23 __attribute__((noreturn));
typedef int TA25 [[gnu::deprecated]];
struct A { virtual void f(); };
struct B : A { void f() override [[deprecated]]; int g() asm("x") [[nodiscard]] __attribute__((unused)) alignas(4); };
[[nodiscard]] void f1();
struct C { [[nodiscard]] C(); [[nodiscard]] ~C(); [[nodiscard]] operator int(); };
[[nodiscard]] int (*fp1)();
[[nodiscard]] void (*fp2)();
struct D { [[nodiscard]] int (*m)(); };
using X1 [[nodiscard]] = int(*)();
using X2 [[nodiscard]] = void(*)();
typedef int X3 __attribute__((warn_unused_result));
using X4 __attribute__((warn_unused_result)) = int;
__attribute__((noreturn)) void (*fp3)();
using X5 __attribute__((noreturn)) = void();
using X6 __attribute__((noreturn)) = void(*)();
using X7 __attribute__((noreturn)) = int&;
struct E { void g() __attribute__((noreturn)); [[noreturn]] void h(); [[no_unique_address]] int k; [[no_unique_address]] static int s; };
[[carries_dependency]] void f2([[carries_dependency]] int);
template<class T> using X9 alignas(8) = T;
template<class T> using X10 [[nodiscard]] = T;
int v [[gnu::warn_unused_result]] [[noreturn]], w [[likely]];
int V1 [[nodiscard]];
void F2() [[nodiscard]];
struct S3 { int m [[nodiscard]]; };
struct B4 { int m [[nodiscard]] : 3; };
typedef int T6 [[nodiscard]];
using U7 [[nodiscard]] = int;
typedef void (*FP8) () [[nodiscard]];
using UF9 [[nodiscard]] = void();
enum [[nodiscard]] E10 {};
struct [[nodiscard]] C11 {};
struct SM13 { static int m [[nodiscard]]; };
int V14 [[noreturn]];
void F15() [[noreturn]];
struct S16 { int m [[noreturn]]; };
struct B17 { int m [[noreturn]] : 3; };
typedef int T19 [[noreturn]];
using U20 [[noreturn]] = int;
typedef void (*FP21) () [[noreturn]];
using UF22 [[noreturn]] = void();
enum [[noreturn]] E23 {};
struct [[noreturn]] C24 {};
struct SM26 { static int m [[noreturn]]; };
int V27 __attribute__((packed));
void F28() __attribute__((packed));
struct S29 { int m __attribute__((packed)); };
typedef int T32 __attribute__((packed));
using U33 __attribute__((packed)) = int;
typedef void (*FP34) () __attribute__((packed));
using UF35 __attribute__((packed)) = void();
enum __attribute__((packed)) E36 {};
struct __attribute__((packed)) C37 {};
struct SM39 { static int m __attribute__((packed)); };
int V40 [[no_unique_address]];
void F41() [[no_unique_address]];
struct S42 { int m [[no_unique_address]]; };
struct B43 { int m [[no_unique_address]] : 3; };
typedef int T45 [[no_unique_address]];
using U46 [[no_unique_address]] = int;
typedef void (*FP47) () [[no_unique_address]];
using UF48 [[no_unique_address]] = void();
enum [[no_unique_address]] E49 {};
struct [[no_unique_address]] C50 {};
struct SM52 { static int m [[no_unique_address]]; };
int V53 [[carries_dependency]];
void F54() [[carries_dependency]];
struct S55 { int m [[carries_dependency]]; };
struct B56 { int m [[carries_dependency]] : 3; };
typedef int T58 [[carries_dependency]];
using U59 [[carries_dependency]] = int;
typedef void (*FP60) () [[carries_dependency]];
using UF61 [[carries_dependency]] = void();
enum [[carries_dependency]] E62 {};
struct [[carries_dependency]] C63 {};
struct SM65 { static int m [[carries_dependency]]; };
int V66 [[fallthrough]];
void F67() [[fallthrough]];
struct S68 { int m [[fallthrough]]; };
struct B69 { int m [[fallthrough]] : 3; };
typedef int T71 [[fallthrough]];
using U72 [[fallthrough]] = int;
typedef void (*FP73) () [[fallthrough]];
using UF74 [[fallthrough]] = void();
enum [[fallthrough]] E75 {};
struct [[fallthrough]] C76 {};
struct SM78 { static int m [[fallthrough]]; };
int V79 [[likely]];
void F80() [[likely]];
struct S81 { int m [[likely]]; };
struct B82 { int m [[likely]] : 3; };
typedef int T84 [[likely]];
using U85 [[likely]] = int;
typedef void (*FP86) () [[likely]];
using UF87 [[likely]] = void();
enum [[likely]] E88 {};
struct [[likely]] C89 {};
struct SM91 { static int m [[likely]]; };
int V92 [[gnu::warn_unused_result]];
void F93() [[gnu::warn_unused_result]];
struct S94 { int m [[gnu::warn_unused_result]]; };
struct B95 { int m [[gnu::warn_unused_result]] : 3; };
typedef int T97 [[gnu::warn_unused_result]];
using U98 [[gnu::warn_unused_result]] = int;
typedef void (*FP99) () [[gnu::warn_unused_result]];
using UF100 [[gnu::warn_unused_result]] = void();
enum [[gnu::warn_unused_result]] E101 {};
struct [[gnu::warn_unused_result]] C102 {};
struct SM104 { static int m [[gnu::warn_unused_result]]; };
int V105 __attribute__((noreturn));
void F106() __attribute__((noreturn));
struct S107 { int m __attribute__((noreturn)); };
typedef int T110 __attribute__((noreturn));
using U111 __attribute__((noreturn)) = int;
typedef void (*FP112) () __attribute__((noreturn));
using UF113 __attribute__((noreturn)) = void();
enum __attribute__((noreturn)) E114 {};
struct __attribute__((noreturn)) C115 {};
struct SM117 { static int m __attribute__((noreturn)); };
int V118 [[maybe_unused]];
void F119() [[maybe_unused]];
struct S120 { int m [[maybe_unused]]; };
struct B121 { int m [[maybe_unused]] : 3; };
typedef int T123 [[maybe_unused]];
using U124 [[maybe_unused]] = int;
typedef void (*FP125) () [[maybe_unused]];
using UF126 [[maybe_unused]] = void();
enum [[maybe_unused]] E127 {};
struct [[maybe_unused]] C128 {};
struct SM130 { static int m [[maybe_unused]]; };
int V131 [[deprecated]];
void F132() [[deprecated]];
struct S133 { int m [[deprecated]]; };
struct B134 { int m [[deprecated]] : 3; };
typedef int T136 [[deprecated]];
using U137 [[deprecated]] = int;
typedef void (*FP138) () [[deprecated]];
using UF139 [[deprecated]] = void();
enum [[deprecated]] E140 {};
struct [[deprecated]] C141 {};
struct SM143 { static int m [[deprecated]]; };
int V144 __attribute__((unused));
void F145() __attribute__((unused));
struct S146 { int m __attribute__((unused)); };
typedef int T149 __attribute__((unused));
using U150 __attribute__((unused)) = int;
typedef void (*FP151) () __attribute__((unused));
using UF152 __attribute__((unused)) = void();
enum __attribute__((unused)) E153 {};
struct __attribute__((unused)) C154 {};
struct SM156 { static int m __attribute__((unused)); };
int V157 [[nodiscard("x")]];
void F158() [[nodiscard("x")]];
struct S159 { int m [[nodiscard("x")]]; };
struct B160 { int m [[nodiscard("x")]] : 3; };
typedef int T162 [[nodiscard("x")]];
using U163 [[nodiscard("x")]] = int;
typedef void (*FP164) () [[nodiscard("x")]];
using UF165 [[nodiscard("x")]] = void();
enum [[nodiscard("x")]] E166 {};
struct [[nodiscard("x")]] C167 {};
struct SM169 { static int m [[nodiscard("x")]]; };
int V170 [[gnu::noreturn]];
void F171() [[gnu::noreturn]];
struct S172 { int m [[gnu::noreturn]]; };
struct B173 { int m [[gnu::noreturn]] : 3; };
typedef int T175 [[gnu::noreturn]];
using U176 [[gnu::noreturn]] = int;
typedef void (*FP177) () [[gnu::noreturn]];
enum [[gnu::noreturn]] E179 {};
struct [[gnu::noreturn]] C180 {};
struct SM182 { static int m [[gnu::noreturn]]; };
