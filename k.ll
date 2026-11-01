; ModuleID = 'tests/codegen/paren_aggregate_init.cpp'
source_filename = "tests/codegen/paren_aggregate_init.cpp"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

%struct.A = type { i32, i32 }
%struct.D = type { %struct.A, [2 x i32] }
%union.U = type { i32 }
%struct.B = type { i32, i64, double }
%struct.C = type { %struct.A, i32 }
%struct.S = type { i32 }
%struct.Z = type { i32, i32 }
%struct.T = type { i32 }

$_Z1kv = comdat any

$_ZN1SC2Ei = comdat any

$_ZN1SC2Ev = comdat any

$_ZN1ZC2Ev = comdat any

$_ZN1TC2Ev = comdat any

@__const._Z1fv.a = private unnamed_addr constant %struct.A { i32 3, i32 0 }, align 4
@__const._Z1fv.b = private unnamed_addr constant %struct.A { i32 1, i32 2 }, align 4
@__const._Z1fv.e = private unnamed_addr constant [3 x i32] [i32 1, i32 2, i32 0], align 4
@__const._Z1fv.h = private unnamed_addr constant [3 x i32] [i32 4, i32 5, i32 6], align 4
@__const._Z1fv.dd = private unnamed_addr constant %struct.D { %struct.A { i32 1, i32 2 }, [2 x i32] zeroinitializer }, align 4
@glob = dso_local global %struct.A { i32 7, i32 8 }, align 4
@garr = dso_local global [4 x i32] [i32 1, i32 0, i32 0, i32 0], align 16
@g1 = dso_local global %union.U { i32 3 }, align 4
@g2 = dso_local global %union.U { i32 2 }, align 4
@__const._Z5use_uv.u = private unnamed_addr constant %union.U { i32 4 }, align 4
@__const._Z1kv.a = private unnamed_addr constant %struct.A { i32 3, i32 4 }, align 4
@__const._Z1kv.arr = private unnamed_addr constant [3 x i32] [i32 1, i32 2, i32 0], align 4

define dso_local void @_Z1fv() #0 {
  %1 = alloca %struct.A, align 4
  %2 = alloca %struct.A, align 4
  %3 = alloca %struct.B, align 8
  %4 = alloca %struct.C, align 4
  %5 = alloca %struct.A, align 4
  %6 = alloca [3 x i32], align 4
  %7 = alloca [3 x i32], align 4
  %8 = alloca %struct.D, align 4
  %9 = alloca %struct.A, align 4
  %10 = alloca ptr, align 8
  %11 = alloca %struct.A, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %1, ptr align 4 @__const._Z1fv.a, i64 8, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 4 @__const._Z1fv.b, i64 8, i1 false)
  %12 = getelementptr inbounds %struct.B, ptr %3, i32 0, i32 0
  store i32 1, ptr %12, align 8
  %13 = getelementptr inbounds %struct.B, ptr %3, i32 0, i32 1
  store i64 2, ptr %13, align 8
  %14 = getelementptr inbounds %struct.B, ptr %3, i32 0, i32 2
  store double 2.500000e+00, ptr %14, align 8
  %15 = getelementptr inbounds %struct.A, ptr %5, i32 0, i32 0
  store i32 1, ptr %15, align 4
  %16 = getelementptr inbounds %struct.A, ptr %5, i32 0, i32 1
  store i32 2, ptr %16, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %4, ptr align 4 %5, i64 8, i1 false)
  %17 = getelementptr inbounds %struct.C, ptr %4, i32 0, i32 1
  store i32 3, ptr %17, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %6, ptr align 4 @__const._Z1fv.e, i64 12, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %7, ptr align 4 @__const._Z1fv.h, i64 12, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %8, ptr align 4 @__const._Z1fv.dd, i64 16, i1 false)
  %18 = getelementptr inbounds %struct.A, ptr %9, i32 0, i32 0
  %19 = call noundef i32 @_Z1gv()
  store i32 %19, ptr %18, align 4
  %20 = getelementptr inbounds %struct.A, ptr %9, i32 0, i32 1
  store i32 4, ptr %20, align 4
  %21 = getelementptr inbounds %struct.A, ptr %11, i32 0, i32 0
  store i32 5, ptr %21, align 4
  %22 = getelementptr inbounds %struct.A, ptr %11, i32 0, i32 1
  store i32 0, ptr %22, align 4
  store ptr %11, ptr %10, align 8
  ret void
}

declare void @llvm.memcpy.p0.p0.i64(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i64, i1 immarg) #1

declare noundef i32 @_Z1gv() #2

define dso_local noundef i32 @_Z5use_kv() #0 {
  %1 = call noundef i32 @_Z1kv()
  ret i32 %1
}

define linkonce_odr dso_local noundef i32 @_Z1kv() #3 comdat {
  %1 = alloca %struct.A, align 4
  %2 = alloca [3 x i32], align 4
  %3 = alloca %struct.B, align 8
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %1, ptr align 4 @__const._Z1kv.a, i64 8, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 4 @__const._Z1kv.arr, i64 12, i1 false)
  %4 = getelementptr inbounds %struct.B, ptr %3, i32 0, i32 0
  store i32 1, ptr %4, align 8
  %5 = getelementptr inbounds %struct.B, ptr %3, i32 0, i32 1
  store i64 0, ptr %5, align 8
  %6 = getelementptr inbounds %struct.B, ptr %3, i32 0, i32 2
  store double 2.500000e+00, ptr %6, align 8
  %7 = getelementptr inbounds %struct.A, ptr %1, i32 0, i32 0
  %8 = load i32, ptr %7, align 4
  %9 = getelementptr inbounds %struct.A, ptr %1, i32 0, i32 1
  %10 = load i32, ptr %9, align 4
  %11 = add nsw i32 %8, %10
  %12 = getelementptr inbounds [3 x i32], ptr %2, i64 0, i64 1
  %13 = load i32, ptr %12, align 4
  %14 = add nsw i32 %11, %13
  %15 = getelementptr inbounds [3 x i32], ptr %2, i64 0, i64 2
  %16 = load i32, ptr %15, align 4
  %17 = add nsw i32 %14, %16
  %18 = getelementptr inbounds %struct.B, ptr %3, i32 0, i32 2
  %19 = load double, ptr %18, align 8
  %20 = fptosi double %19 to i32
  %21 = add nsw i32 %17, %20
  ret i32 %21
}

define dso_local noundef i32 @_Z5use_uv() #3 {
  %1 = alloca %union.U, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %1, ptr align 4 @__const._Z5use_uv.u, i64 4, i1 false)
  %2 = load i32, ptr %1, align 4
  ret i32 %2
}

define dso_local noundef float @_Z6use_ufi(i32 noundef %0) #3 {
  %2 = alloca i32, align 4
  %3 = alloca %union.U, align 4
  store i32 %0, ptr %2, align 4
  %4 = load i32, ptr %2, align 4
  store i32 %4, ptr %3, align 4
  %5 = load float, ptr %3, align 4
  ret float %5
}

define dso_local noundef i32 @_Z7use_arrv() #0 personality ptr @__gxx_personality_v0 {
  %1 = alloca i32, align 4
  %2 = alloca [3 x %struct.S], align 4
  %3 = alloca ptr, align 8
  %4 = alloca ptr, align 8
  %5 = alloca i32, align 4
  store ptr %2, ptr %3, align 8
  invoke void @_ZN1SC2Ei(ptr noundef nonnull align 4 dereferenceable(4) %2, i32 noundef 1)
          to label %6 unwind label %20

6:
  %7 = getelementptr inbounds %struct.S, ptr %2, i64 1
  store ptr %7, ptr %3, align 8
  %8 = getelementptr inbounds %struct.S, ptr %2, i64 3
  br label %9

9:
  %10 = phi ptr [ %7, %6 ], [ %12, %11 ]
  invoke void @_ZN1SC2Ev(ptr noundef nonnull align 4 dereferenceable(4) %10)
          to label %11 unwind label %20

11:
  %12 = getelementptr inbounds %struct.S, ptr %10, i64 1
  store ptr %12, ptr %3, align 8
  %13 = icmp eq ptr %12, %8
  br i1 %13, label %14, label %9

14:
  %15 = getelementptr inbounds [3 x %struct.S], ptr %2, i64 0, i64 2
  %16 = getelementptr inbounds %struct.S, ptr %15, i32 0, i32 0
  %17 = load i32, ptr %16, align 4
  store i32 %17, ptr %1, align 4
  %18 = getelementptr inbounds [3 x %struct.S], ptr %2, i32 0, i32 0
  %19 = getelementptr inbounds %struct.S, ptr %18, i64 3
  br label %31

20:
  %21 = landingpad { ptr, i32 }
          cleanup
  %22 = extractvalue { ptr, i32 } %21, 0
  store ptr %22, ptr %4, align 8
  %23 = extractvalue { ptr, i32 } %21, 1
  store i32 %23, ptr %5, align 4
  %24 = load ptr, ptr %3, align 8
  %25 = icmp eq ptr %2, %24
  br i1 %25, label %30, label %26

26:
  %27 = phi ptr [ %24, %20 ], [ %28, %26 ]
  %28 = getelementptr inbounds %struct.S, ptr %27, i64 -1
  call void @_ZN1SD1Ev(ptr noundef nonnull align 4 dereferenceable(4) %28) #9
  %29 = icmp eq ptr %28, %2
  br i1 %29, label %30, label %26

30:
  br label %37

31:
  %32 = phi ptr [ %19, %14 ], [ %33, %31 ]
  %33 = getelementptr inbounds %struct.S, ptr %32, i64 -1
  call void @_ZN1SD1Ev(ptr noundef nonnull align 4 dereferenceable(4) %33) #9
  %34 = icmp eq ptr %33, %18
  br i1 %34, label %35, label %31

35:
  %36 = load i32, ptr %1, align 4
  ret i32 %36

37:
  %38 = load ptr, ptr %4, align 8
  %39 = load i32, ptr %5, align 4
  %40 = insertvalue { ptr, i32 } poison, ptr %38, 0
  %41 = insertvalue { ptr, i32 } %40, i32 %39, 1
  resume { ptr, i32 } %41
}

define linkonce_odr dso_local void @_ZN1SC2Ei(ptr noundef nonnull align 4 dereferenceable(4) %0, i32 noundef %1) unnamed_addr #3 comdat align 2 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  store ptr %0, ptr %3, align 8
  store i32 %1, ptr %4, align 4
  %5 = load ptr, ptr %3, align 8
  %6 = getelementptr inbounds %struct.S, ptr %5, i32 0, i32 0
  %7 = load i32, ptr %4, align 4
  store i32 %7, ptr %6, align 4
  ret void
}

declare i32 @__gxx_personality_v0(...)

define linkonce_odr dso_local void @_ZN1SC2Ev(ptr noundef nonnull align 4 dereferenceable(4) %0) unnamed_addr #3 comdat align 2 {
  %2 = alloca ptr, align 8
  store ptr %0, ptr %2, align 8
  %3 = load ptr, ptr %2, align 8
  %4 = getelementptr inbounds %struct.S, ptr %3, i32 0, i32 0
  store i32 7, ptr %4, align 4
  ret void
}

declare void @_ZN1SD1Ev(ptr noundef nonnull align 4 dereferenceable(4)) unnamed_addr #4

define dso_local noundef i32 @_Z5use_zv() #3 {
  %1 = alloca [2 x %struct.Z], align 16
  %2 = getelementptr inbounds %struct.Z, ptr %1, i32 0, i32 0
  store i32 1, ptr %2, align 8
  %3 = getelementptr inbounds %struct.Z, ptr %1, i32 0, i32 1
  store i32 2, ptr %3, align 4
  %4 = getelementptr inbounds %struct.Z, ptr %1, i64 1
  %5 = getelementptr inbounds %struct.Z, ptr %1, i64 2
  br label %6

6:
  %7 = phi ptr [ %4, %0 ], [ %8, %6 ]
  call void @llvm.memset.p0.i64(ptr align 8 %7, i8 0, i64 8, i1 false)
  call void @_ZN1ZC2Ev(ptr noundef nonnull align 4 dereferenceable(8) %7) #9
  %8 = getelementptr inbounds %struct.Z, ptr %7, i64 1
  %9 = icmp eq ptr %8, %5
  br i1 %9, label %10, label %6

10:
  %11 = getelementptr inbounds [2 x %struct.Z], ptr %1, i64 0, i64 1
  %12 = getelementptr inbounds %struct.Z, ptr %11, i32 0, i32 1
  %13 = load i32, ptr %12, align 4
  ret i32 %13
}

declare void @llvm.memset.p0.i64(ptr nocapture writeonly, i8, i64, i1 immarg) #5

define linkonce_odr dso_local void @_ZN1ZC2Ev(ptr noundef nonnull align 4 dereferenceable(8) %0) unnamed_addr #3 comdat align 2 {
  %2 = alloca ptr, align 8
  store ptr %0, ptr %2, align 8
  %3 = load ptr, ptr %2, align 8
  %4 = getelementptr inbounds %struct.Z, ptr %3, i32 0, i32 1
  store i32 5, ptr %4, align 4
  ret void
}

define dso_local noundef ptr @_Z5new_av() #0 {
  %1 = call noalias noundef nonnull ptr @_Znam(i64 noundef 12) #10
  store i32 1, ptr %1, align 4
  %2 = getelementptr inbounds i32, ptr %1, i64 1
  store i32 2, ptr %2, align 4
  %3 = getelementptr inbounds i32, ptr %2, i64 1
  call void @llvm.memset.p0.i64(ptr align 4 %3, i8 0, i64 4, i1 false)
  ret ptr %1
}

declare noundef nonnull ptr @_Znam(i64 noundef) #6

define dso_local noundef ptr @_Z5new_bi(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  %3 = load i32, ptr %2, align 4
  %4 = sext i32 %3 to i64
  %5 = icmp ult i64 %4, 2
  %6 = call { i64, i1 } @llvm.umul.with.overflow.i64(i64 %4, i64 4)
  %7 = extractvalue { i64, i1 } %6, 1
  %8 = or i1 %5, %7
  %9 = extractvalue { i64, i1 } %6, 0
  %10 = select i1 %8, i64 -1, i64 %9
  %11 = call noalias noundef nonnull ptr @_Znam(i64 noundef %10) #10
  store i32 1, ptr %11, align 4
  %12 = getelementptr inbounds i32, ptr %11, i64 1
  store i32 2, ptr %12, align 4
  %13 = getelementptr inbounds i32, ptr %12, i64 1
  %14 = sub i64 %10, 8
  call void @llvm.memset.p0.i64(ptr align 4 %13, i8 0, i64 %14, i1 false)
  ret ptr %11
}

declare { i64, i1 } @llvm.umul.with.overflow.i64(i64, i64) #7

define dso_local noundef ptr @_Z5new_cv() #0 {
  %1 = call noalias noundef nonnull ptr @_Znam(i64 noundef 12) #10
  store i32 1, ptr %1, align 4
  %2 = getelementptr inbounds i32, ptr %1, i64 1
  store i32 2, ptr %2, align 4
  %3 = getelementptr inbounds i32, ptr %2, i64 1
  store i32 3, ptr %3, align 4
  %4 = getelementptr inbounds i32, ptr %3, i64 1
  ret ptr %1
}

define dso_local noundef ptr @_Z5new_di(i32 noundef %0) #0 personality ptr @__gxx_personality_v0 {
  %2 = alloca i32, align 4
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  %5 = load i32, ptr %2, align 4
  %6 = sext i32 %5 to i64
  %7 = icmp ult i64 %6, 1
  %8 = call { i64, i1 } @llvm.umul.with.overflow.i64(i64 %6, i64 4)
  %9 = extractvalue { i64, i1 } %8, 1
  %10 = or i1 %7, %9
  %11 = extractvalue { i64, i1 } %8, 0
  %12 = select i1 %10, i64 -1, i64 %11
  %13 = call noalias noundef nonnull ptr @_Znam(i64 noundef %12) #10
  invoke void @_ZN1TC2Ev(ptr noundef nonnull align 4 dereferenceable(4) %13)
          to label %14 unwind label %26

14:
  %15 = getelementptr inbounds %struct.T, ptr %13, i64 1
  %16 = sub i64 %6, 1
  %17 = icmp eq i64 %16, 0
  br i1 %17, label %25, label %18

18:
  %19 = getelementptr inbounds %struct.T, ptr %15, i64 %16
  br label %20

20:
  %21 = phi ptr [ %15, %18 ], [ %23, %22 ]
  invoke void @_ZN1TC2Ev(ptr noundef nonnull align 4 dereferenceable(4) %21)
          to label %22 unwind label %26

22:
  %23 = getelementptr inbounds %struct.T, ptr %21, i64 1
  %24 = icmp eq ptr %23, %19
  br i1 %24, label %25, label %20

25:
  ret ptr %13

26:
  %27 = landingpad { ptr, i32 }
          cleanup
  %28 = extractvalue { ptr, i32 } %27, 0
  store ptr %28, ptr %3, align 8
  %29 = extractvalue { ptr, i32 } %27, 1
  store i32 %29, ptr %4, align 4
  call void @_ZdaPvm(ptr noundef %13, i64 noundef %12) #11
  br label %30

30:
  %31 = load ptr, ptr %3, align 8
  %32 = load i32, ptr %4, align 4
  %33 = insertvalue { ptr, i32 } poison, ptr %31, 0
  %34 = insertvalue { ptr, i32 } %33, i32 %32, 1
  resume { ptr, i32 } %34
}

define linkonce_odr dso_local void @_ZN1TC2Ev(ptr noundef nonnull align 4 dereferenceable(4) %0) unnamed_addr #3 comdat align 2 {
  %2 = alloca ptr, align 8
  store ptr %0, ptr %2, align 8
  %3 = load ptr, ptr %2, align 8
  %4 = getelementptr inbounds %struct.T, ptr %3, i32 0, i32 0
  store i32 3, ptr %4, align 4
  ret void
}

declare void @_ZdaPvm(ptr noundef, i64 noundef) #8

define dso_local noundef ptr @_Z5new_ev() #0 {
  %1 = call noalias noundef nonnull ptr @_Znwm(i64 noundef 8) #10
  %2 = getelementptr inbounds %struct.Z, ptr %1, i32 0, i32 0
  store i32 9, ptr %2, align 8
  %3 = getelementptr inbounds %struct.Z, ptr %1, i32 0, i32 1
  store i32 5, ptr %3, align 4
  ret ptr %1
}

declare noundef nonnull ptr @_Znwm(i64 noundef) #6

attributes #0 = { mustprogress noinline optnone uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { nocallback nofree nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #3 = { mustprogress noinline nounwind optnone uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #4 = { nounwind "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #5 = { nocallback nofree nounwind willreturn memory(argmem: write) }
attributes #6 = { nobuiltin allocsize(0) "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #7 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #8 = { nobuiltin nounwind "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #9 = { nounwind }
attributes #10 = { builtin allocsize(0) }
attributes #11 = { builtin nounwind }

!llvm.linker.options = !{}
!llvm.module.flags = !{!0, !1, !2, !3, !4}
!llvm.ident = !{!5}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"uwtable", i32 2}
!4 = !{i32 7, !"frame-pointer", i32 2}
!5 = !{!"kiln version 0.0.1"}
