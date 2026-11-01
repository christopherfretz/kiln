; ModuleID = 'tests/codegen/eh_template_try.cpp'
source_filename = "tests/codegen/eh_template_try.cpp"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

%class.anon = type { i8 }
%struct.S = type { i64 }
%struct.C = type { i32 }

$_ZN1SIlEC2El = comdat any

$_Z1fIiEiT_ = comdat any

$_Z1fIdEiT_ = comdat any

$_Z3ftbIiEiT_ = comdat any

$_Z3lamIsEiv = comdat any

$_Z6nestedI1CEiv = comdat any

$__clang_call_terminate = comdat any

$_ZZ3lamIsEivENKUlT_E_clIsEEDaS0_ = comdat any

$_ZTS1C = comdat any

$_ZTI1C = comdat any

@_ZTIl = external constant ptr
@_ZTIi = external constant ptr
@_ZTId = external constant ptr
@__const._Z3lamIsEiv.l = private unnamed_addr constant %class.anon undef, align 1
@_ZTIs = external constant ptr
@_ZTVN10__cxxabiv117__class_type_infoE = external global [0 x ptr]
@_ZTS1C = linkonce_odr dso_local constant [3 x i8] c"1C\00", comdat, align 1
@_ZTI1C = linkonce_odr dso_local constant { ptr, ptr } { ptr getelementptr inbounds (ptr, ptr @_ZTVN10__cxxabiv117__class_type_infoE, i64 2), ptr @_ZTS1C }, comdat, align 8

; Function Attrs: mustprogress noinline optnone uwtable
define dso_local noundef i32 @_Z3usev() #0 {
  %1 = alloca %struct.S, align 8
  call void @_ZN1SIlEC2El(ptr noundef nonnull align 8 dereferenceable(8) %1, i64 noundef 2)
  %2 = call noundef i32 @_Z1fIiEiT_(i32 noundef 1)
  %3 = call noundef i32 @_Z1fIdEiT_(double noundef 2.000000e+00)
  %4 = add nsw i32 %2, %3
  %5 = call noundef i32 @_Z3ftbIiEiT_(i32 noundef 3)
  %6 = add nsw i32 %4, %5
  %7 = call noundef i32 @_Z3lamIsEiv()
  %8 = add nsw i32 %6, %7
  %9 = call noundef i32 @_Z6nestedI1CEiv()
  %10 = add nsw i32 %8, %9
  ret i32 %10
}

; Function Attrs: mustprogress noinline optnone uwtable
define linkonce_odr dso_local void @_ZN1SIlEC2El(ptr noundef nonnull align 8 dereferenceable(8) %0, i64 noundef %1) unnamed_addr #0 comdat align 2 personality ptr @__gxx_personality_v0 {
  %3 = alloca ptr, align 8
  %4 = alloca i64, align 8
  %5 = alloca ptr, align 8
  %6 = alloca i32, align 4
  %7 = alloca ptr, align 8
  store ptr %0, ptr %3, align 8
  store i64 %1, ptr %4, align 8
  %8 = load ptr, ptr %3, align 8
  %9 = getelementptr inbounds %struct.S, ptr %8, i32 0, i32 0
  %10 = load i64, ptr %4, align 8
  store i64 %10, ptr %9, align 8
  %11 = invoke noundef i32 @_Z1gi(i32 noundef 1)
          to label %12 unwind label %13

12:                                               ; preds = %2
  br label %31

13:                                               ; preds = %2
  %14 = landingpad { ptr, i32 }
          catch ptr @_ZTIl
  %15 = extractvalue { ptr, i32 } %14, 0
  store ptr %15, ptr %5, align 8
  %16 = extractvalue { ptr, i32 } %14, 1
  store i32 %16, ptr %6, align 4
  br label %17

17:                                               ; preds = %13
  %18 = load i32, ptr %6, align 4
  %19 = call i32 @llvm.eh.typeid.for.p0(ptr @_ZTIl) #7
  %20 = icmp eq i32 %18, %19
  br i1 %20, label %21, label %32

21:                                               ; preds = %17
  %22 = load ptr, ptr %5, align 8
  %23 = call ptr @__cxa_begin_catch(ptr %22) #7
  store ptr %23, ptr %7, align 8
  %24 = invoke noundef i32 @_Z1gi(i32 noundef 2)
          to label %25 unwind label %27

25:                                               ; preds = %21
  invoke void @__cxa_rethrow()
          to label %26 unwind label %27

26:                                               ; preds = %25
  unreachable

27:                                               ; preds = %25, %21
  %28 = landingpad { ptr, i32 }
          cleanup
  %29 = extractvalue { ptr, i32 } %28, 0
  store ptr %29, ptr %5, align 8
  %30 = extractvalue { ptr, i32 } %28, 1
  store i32 %30, ptr %6, align 4
  call void @__cxa_end_catch() #7
  br label %32

31:                                               ; preds = %12
  ret void

32:                                               ; preds = %27, %17
  %33 = load ptr, ptr %5, align 8
  %34 = load i32, ptr %6, align 4
  %35 = insertvalue { ptr, i32 } poison, ptr %33, 0
  %36 = insertvalue { ptr, i32 } %35, i32 %34, 1
  resume { ptr, i32 } %36
}

; Function Attrs: mustprogress noinline optnone uwtable
define linkonce_odr dso_local noundef i32 @_Z1fIiEiT_(i32 noundef %0) #0 comdat personality ptr @__gxx_personality_v0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca ptr, align 8
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  %7 = load i32, ptr %3, align 4
  %8 = icmp ne i32 %7, 0
  br i1 %8, label %9, label %30

9:                                                ; preds = %1
  %10 = call ptr @__cxa_allocate_exception(i64 4) #7
  %11 = load i32, ptr %3, align 4
  store i32 %11, ptr %10, align 16
  invoke void @__cxa_throw(ptr %10, ptr @_ZTIi, ptr null) #8
          to label %43 unwind label %12

12:                                               ; preds = %9
  %13 = landingpad { ptr, i32 }
          catch ptr @_ZTIi
          catch ptr null
  %14 = extractvalue { ptr, i32 } %13, 0
  store ptr %14, ptr %4, align 8
  %15 = extractvalue { ptr, i32 } %13, 1
  store i32 %15, ptr %5, align 4
  br label %16

16:                                               ; preds = %12
  %17 = load i32, ptr %5, align 4
  %18 = call i32 @llvm.eh.typeid.for.p0(ptr @_ZTIi) #7
  %19 = icmp eq i32 %17, %18
  br i1 %19, label %20, label %27

20:                                               ; preds = %16
  %21 = load ptr, ptr %4, align 8
  %22 = call ptr @__cxa_begin_catch(ptr %21) #7
  %23 = load i32, ptr %22, align 4
  store i32 %23, ptr %6, align 4
  %24 = load i32, ptr %6, align 4
  %25 = invoke noundef i32 @_Z1gi(i32 noundef %24)
          to label %26 unwind label %31

26:                                               ; preds = %20
  store i32 %25, ptr %2, align 4
  call void @__cxa_end_catch() #7
  br label %36

27:                                               ; preds = %16
  %28 = load ptr, ptr %4, align 8
  %29 = call ptr @__cxa_begin_catch(ptr %28) #7
  store i32 3, ptr %2, align 4
  call void @__cxa_end_catch()
  br label %36

30:                                               ; preds = %1
  br label %35

31:                                               ; preds = %20
  %32 = landingpad { ptr, i32 }
          cleanup
  %33 = extractvalue { ptr, i32 } %32, 0
  store ptr %33, ptr %4, align 8
  %34 = extractvalue { ptr, i32 } %32, 1
  store i32 %34, ptr %5, align 4
  call void @__cxa_end_catch() #7
  br label %38

35:                                               ; preds = %30
  store i32 0, ptr %2, align 4
  br label %36

36:                                               ; preds = %35, %26, %27
  %37 = load i32, ptr %2, align 4
  ret i32 %37

38:                                               ; preds = %31
  %39 = load ptr, ptr %4, align 8
  %40 = load i32, ptr %5, align 4
  %41 = insertvalue { ptr, i32 } poison, ptr %39, 0
  %42 = insertvalue { ptr, i32 } %41, i32 %40, 1
  resume { ptr, i32 } %42

43:                                               ; preds = %9
  unreachable
}

; Function Attrs: mustprogress noinline optnone uwtable
define linkonce_odr dso_local noundef i32 @_Z1fIdEiT_(double noundef %0) #0 comdat personality ptr @__gxx_personality_v0 {
  %2 = alloca i32, align 4
  %3 = alloca double, align 8
  %4 = alloca ptr, align 8
  %5 = alloca i32, align 4
  %6 = alloca double, align 8
  store double %0, ptr %3, align 8
  %7 = load double, ptr %3, align 8
  %8 = fcmp une double %7, 0.000000e+00
  br i1 %8, label %9, label %31

9:                                                ; preds = %1
  %10 = call ptr @__cxa_allocate_exception(i64 8) #7
  %11 = load double, ptr %3, align 8
  store double %11, ptr %10, align 16
  invoke void @__cxa_throw(ptr %10, ptr @_ZTId, ptr null) #8
          to label %44 unwind label %12

12:                                               ; preds = %9
  %13 = landingpad { ptr, i32 }
          catch ptr @_ZTId
          catch ptr null
  %14 = extractvalue { ptr, i32 } %13, 0
  store ptr %14, ptr %4, align 8
  %15 = extractvalue { ptr, i32 } %13, 1
  store i32 %15, ptr %5, align 4
  br label %16

16:                                               ; preds = %12
  %17 = load i32, ptr %5, align 4
  %18 = call i32 @llvm.eh.typeid.for.p0(ptr @_ZTId) #7
  %19 = icmp eq i32 %17, %18
  br i1 %19, label %20, label %28

20:                                               ; preds = %16
  %21 = load ptr, ptr %4, align 8
  %22 = call ptr @__cxa_begin_catch(ptr %21) #7
  %23 = load double, ptr %22, align 8
  store double %23, ptr %6, align 8
  %24 = load double, ptr %6, align 8
  %25 = fptosi double %24 to i32
  %26 = invoke noundef i32 @_Z1gi(i32 noundef %25)
          to label %27 unwind label %32

27:                                               ; preds = %20
  store i32 %26, ptr %2, align 4
  call void @__cxa_end_catch() #7
  br label %37

28:                                               ; preds = %16
  %29 = load ptr, ptr %4, align 8
  %30 = call ptr @__cxa_begin_catch(ptr %29) #7
  store i32 3, ptr %2, align 4
  call void @__cxa_end_catch()
  br label %37

31:                                               ; preds = %1
  br label %36

32:                                               ; preds = %20
  %33 = landingpad { ptr, i32 }
          cleanup
  %34 = extractvalue { ptr, i32 } %33, 0
  store ptr %34, ptr %4, align 8
  %35 = extractvalue { ptr, i32 } %33, 1
  store i32 %35, ptr %5, align 4
  call void @__cxa_end_catch() #7
  br label %39

36:                                               ; preds = %31
  store i32 0, ptr %2, align 4
  br label %37

37:                                               ; preds = %36, %27, %28
  %38 = load i32, ptr %2, align 4
  ret i32 %38

39:                                               ; preds = %32
  %40 = load ptr, ptr %4, align 8
  %41 = load i32, ptr %5, align 4
  %42 = insertvalue { ptr, i32 } poison, ptr %40, 0
  %43 = insertvalue { ptr, i32 } %42, i32 %41, 1
  resume { ptr, i32 } %43

44:                                               ; preds = %9
  unreachable
}

; Function Attrs: mustprogress noinline optnone uwtable
define linkonce_odr dso_local noundef i32 @_Z3ftbIiEiT_(i32 noundef %0) #0 comdat personality ptr @__gxx_personality_v0 {
  %2 = alloca i32, align 4
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  %5 = alloca ptr, align 8
  store i32 %0, ptr %2, align 4
  %6 = call ptr @__cxa_allocate_exception(i64 4) #7
  %7 = load i32, ptr %2, align 4
  store i32 %7, ptr %6, align 16
  invoke void @__cxa_throw(ptr %6, ptr @_ZTIi, ptr null) #8
          to label %38 unwind label %8

8:                                                ; preds = %1
  %9 = landingpad { ptr, i32 }
          catch ptr @_ZTIi
          catch ptr null
  %10 = extractvalue { ptr, i32 } %9, 0
  store ptr %10, ptr %3, align 8
  %11 = extractvalue { ptr, i32 } %9, 1
  store i32 %11, ptr %4, align 4
  br label %12

12:                                               ; preds = %8
  %13 = load i32, ptr %4, align 4
  %14 = call i32 @llvm.eh.typeid.for.p0(ptr @_ZTIi) #7
  %15 = icmp eq i32 %13, %14
  br i1 %15, label %16, label %21

16:                                               ; preds = %12
  %17 = load ptr, ptr %3, align 8
  %18 = call ptr @__cxa_begin_catch(ptr %17) #7
  store ptr %18, ptr %5, align 8
  %19 = load ptr, ptr %5, align 8
  %20 = load i32, ptr %19, align 4
  call void @__cxa_end_catch() #7
  ret i32 %20

21:                                               ; preds = %12
  %22 = load ptr, ptr %3, align 8
  %23 = call ptr @__cxa_begin_catch(ptr %22) #7
  invoke void @__cxa_rethrow() #8
          to label %38 unwind label %24

24:                                               ; preds = %21
  %25 = landingpad { ptr, i32 }
          cleanup
  %26 = extractvalue { ptr, i32 } %25, 0
  store ptr %26, ptr %3, align 8
  %27 = extractvalue { ptr, i32 } %25, 1
  store i32 %27, ptr %4, align 4
  invoke void @__cxa_end_catch()
          to label %28 unwind label %35

28:                                               ; preds = %24
  br label %30

29:                                               ; No predecessors!
  call void @llvm.trap()
  unreachable

30:                                               ; preds = %28
  %31 = load ptr, ptr %3, align 8
  %32 = load i32, ptr %4, align 4
  %33 = insertvalue { ptr, i32 } poison, ptr %31, 0
  %34 = insertvalue { ptr, i32 } %33, i32 %32, 1
  resume { ptr, i32 } %34

35:                                               ; preds = %24
  %36 = landingpad { ptr, i32 }
          catch ptr null
  %37 = extractvalue { ptr, i32 } %36, 0
  call void @__clang_call_terminate(ptr %37) #9
  unreachable

38:                                               ; preds = %21, %1
  unreachable
}

; Function Attrs: mustprogress noinline optnone uwtable
define linkonce_odr dso_local noundef i32 @_Z3lamIsEiv() #0 comdat {
  %1 = alloca %class.anon, align 1
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %1, ptr align 1 @__const._Z3lamIsEiv.l, i64 1, i1 false)
  %2 = call noundef i32 @_ZZ3lamIsEivENKUlT_E_clIsEEDaS0_(ptr noundef nonnull align 1 dereferenceable(1) %1, i16 noundef signext 1)
  ret i32 %2
}

; Function Attrs: mustprogress noinline optnone uwtable
define linkonce_odr dso_local noundef i32 @_Z6nestedI1CEiv() #0 comdat personality ptr @__gxx_personality_v0 {
  %1 = alloca i32, align 4
  %2 = alloca ptr, align 8
  %3 = alloca i32, align 4
  %4 = alloca ptr, align 8
  %5 = alloca %struct.C, align 4
  %6 = call ptr @__cxa_allocate_exception(i64 4) #7
  invoke void @_ZN1CC1Ev(ptr noundef nonnull align 4 dereferenceable(4) %6)
          to label %7 unwind label %8

7:                                                ; preds = %0
  invoke void @__cxa_throw(ptr %6, ptr @_ZTI1C, ptr @_ZN1CD1Ev) #8
          to label %51 unwind label %12

8:                                                ; preds = %0
  %9 = landingpad { ptr, i32 }
          cleanup
          catch ptr @_ZTI1C
  %10 = extractvalue { ptr, i32 } %9, 0
  store ptr %10, ptr %2, align 8
  %11 = extractvalue { ptr, i32 } %9, 1
  store i32 %11, ptr %3, align 4
  call void @__cxa_free_exception(ptr %6) #7
  br label %16

12:                                               ; preds = %7
  %13 = landingpad { ptr, i32 }
          catch ptr @_ZTI1C
  %14 = extractvalue { ptr, i32 } %13, 0
  store ptr %14, ptr %2, align 8
  %15 = extractvalue { ptr, i32 } %13, 1
  store i32 %15, ptr %3, align 4
  br label %16

16:                                               ; preds = %12, %8
  %17 = load i32, ptr %3, align 4
  %18 = call i32 @llvm.eh.typeid.for.p0(ptr @_ZTI1C) #7
  %19 = icmp eq i32 %17, %18
  br i1 %19, label %20, label %28

20:                                               ; preds = %16
  %21 = load ptr, ptr %2, align 8
  %22 = call ptr @__cxa_begin_catch(ptr %21) #7
  store ptr %22, ptr %4, align 8
  invoke void @__cxa_rethrow() #8
          to label %51 unwind label %23

23:                                               ; preds = %20
  %24 = landingpad { ptr, i32 }
          cleanup
          catch ptr @_ZTI1C
  %25 = extractvalue { ptr, i32 } %24, 0
  store ptr %25, ptr %2, align 8
  %26 = extractvalue { ptr, i32 } %24, 1
  store i32 %26, ptr %3, align 4
  invoke void @__cxa_end_catch()
          to label %27 unwind label %48

27:                                               ; preds = %23
  br label %28

28:                                               ; preds = %27, %16
  %29 = load i32, ptr %3, align 4
  %30 = call i32 @llvm.eh.typeid.for.p0(ptr @_ZTI1C) #7
  %31 = icmp eq i32 %29, %30
  br i1 %31, label %32, label %43

32:                                               ; preds = %28
  %33 = load ptr, ptr %2, align 8
  %34 = call ptr @__cxa_get_exception_ptr(ptr %33) #7
  invoke void @_ZN1CC1ERKS_(ptr noundef nonnull align 4 dereferenceable(4) %5, ptr noundef nonnull align 4 dereferenceable(4) %34)
          to label %35 unwind label %48

35:                                               ; preds = %32
  %36 = call ptr @__cxa_begin_catch(ptr %33) #7
  %37 = getelementptr inbounds %struct.C, ptr %5, i32 0, i32 0
  %38 = load i32, ptr %37, align 4
  store i32 %38, ptr %1, align 4
  call void @_ZN1CD1Ev(ptr noundef nonnull align 4 dereferenceable(4) %5) #7
  call void @__cxa_end_catch()
  br label %41

39:                                               ; No predecessors!
  br label %40

40:                                               ; preds = %39
  store i32 0, ptr %1, align 4
  br label %41

41:                                               ; preds = %40, %35
  %42 = load i32, ptr %1, align 4
  ret i32 %42

43:                                               ; preds = %28
  %44 = load ptr, ptr %2, align 8
  %45 = load i32, ptr %3, align 4
  %46 = insertvalue { ptr, i32 } poison, ptr %44, 0
  %47 = insertvalue { ptr, i32 } %46, i32 %45, 1
  resume { ptr, i32 } %47

48:                                               ; preds = %32, %23
  %49 = landingpad { ptr, i32 }
          catch ptr null
  %50 = extractvalue { ptr, i32 } %49, 0
  call void @__clang_call_terminate(ptr %50) #9
  unreachable

51:                                               ; preds = %20, %7
  unreachable
}

declare noundef i32 @_Z1gi(i32 noundef) #1

declare i32 @__gxx_personality_v0(...)

; Function Attrs: nounwind memory(none)
declare i32 @llvm.eh.typeid.for.p0(ptr) #2

declare ptr @__cxa_begin_catch(ptr)

declare void @__cxa_rethrow()

declare void @__cxa_end_catch()

declare ptr @__cxa_allocate_exception(i64)

declare void @__cxa_throw(ptr, ptr, ptr)

; Function Attrs: noinline noreturn nounwind uwtable
define linkonce_odr hidden void @__clang_call_terminate(ptr noundef %0) #3 comdat {
  %2 = call ptr @__cxa_begin_catch(ptr %0) #7
  call void @_ZSt9terminatev() #9
  unreachable
}

declare void @_ZSt9terminatev()

; Function Attrs: cold noreturn nounwind memory(inaccessiblemem: write)
declare void @llvm.trap() #4

; Function Attrs: nocallback nofree nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i64, i1 immarg) #5

; Function Attrs: mustprogress noinline optnone uwtable
define linkonce_odr dso_local noundef i32 @_ZZ3lamIsEivENKUlT_E_clIsEEDaS0_(ptr noundef nonnull align 1 dereferenceable(1) %0, i16 noundef signext %1) #0 comdat align 2 personality ptr @__gxx_personality_v0 {
  %3 = alloca i32, align 4
  %4 = alloca ptr, align 8
  %5 = alloca i16, align 2
  %6 = alloca ptr, align 8
  %7 = alloca i32, align 4
  %8 = alloca i16, align 2
  store ptr %0, ptr %4, align 8
  store i16 %1, ptr %5, align 2
  %9 = load ptr, ptr %4, align 8
  %10 = call ptr @__cxa_allocate_exception(i64 2) #7
  %11 = load i16, ptr %5, align 2
  store i16 %11, ptr %10, align 16
  invoke void @__cxa_throw(ptr %10, ptr @_ZTIs, ptr null) #8
          to label %34 unwind label %12

12:                                               ; preds = %2
  %13 = landingpad { ptr, i32 }
          catch ptr @_ZTIs
  %14 = extractvalue { ptr, i32 } %13, 0
  store ptr %14, ptr %6, align 8
  %15 = extractvalue { ptr, i32 } %13, 1
  store i32 %15, ptr %7, align 4
  br label %16

16:                                               ; preds = %12
  %17 = load i32, ptr %7, align 4
  %18 = call i32 @llvm.eh.typeid.for.p0(ptr @_ZTIs) #7
  %19 = icmp eq i32 %17, %18
  br i1 %19, label %20, label %29

20:                                               ; preds = %16
  %21 = load ptr, ptr %6, align 8
  %22 = call ptr @__cxa_begin_catch(ptr %21) #7
  %23 = load i16, ptr %22, align 2
  store i16 %23, ptr %8, align 2
  %24 = load i16, ptr %8, align 2
  %25 = sext i16 %24 to i32
  store i32 %25, ptr %3, align 4
  call void @__cxa_end_catch() #7
  br label %27

26:                                               ; No predecessors!
  store i32 0, ptr %3, align 4
  br label %27

27:                                               ; preds = %26, %20
  %28 = load i32, ptr %3, align 4
  ret i32 %28

29:                                               ; preds = %16
  %30 = load ptr, ptr %6, align 8
  %31 = load i32, ptr %7, align 4
  %32 = insertvalue { ptr, i32 } poison, ptr %30, 0
  %33 = insertvalue { ptr, i32 } %32, i32 %31, 1
  resume { ptr, i32 } %33

34:                                               ; preds = %2
  unreachable
}

declare void @_ZN1CC1Ev(ptr noundef nonnull align 4 dereferenceable(4)) unnamed_addr #1

declare void @__cxa_free_exception(ptr)

; Function Attrs: nounwind
declare void @_ZN1CD1Ev(ptr noundef nonnull align 4 dereferenceable(4)) unnamed_addr #6

declare ptr @__cxa_get_exception_ptr(ptr)

declare void @_ZN1CC1ERKS_(ptr noundef nonnull align 4 dereferenceable(4), ptr noundef nonnull align 4 dereferenceable(4)) unnamed_addr #1

attributes #0 = { mustprogress noinline optnone uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #2 = { nounwind memory(none) }
attributes #3 = { noinline noreturn nounwind uwtable "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #4 = { cold noreturn nounwind memory(inaccessiblemem: write) }
attributes #5 = { nocallback nofree nounwind willreturn memory(argmem: readwrite) }
attributes #6 = { nounwind "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #7 = { nounwind }
attributes #8 = { noreturn }
attributes #9 = { noreturn nounwind }

!llvm.linker.options = !{}
!llvm.module.flags = !{!0, !1, !2, !3, !4}
!llvm.ident = !{!5}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"uwtable", i32 2}
!4 = !{i32 7, !"frame-pointer", i32 2}
!5 = !{!"Debian clang version 19.1.7 (3+b1)"}
