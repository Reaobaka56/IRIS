; IRIS Complete LLVM IR — phase 49
; Struct/array types lowered, typed calls, alloca for fixed arrays.

target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-windows-gnu"

%Dual = type { i64, i64 }

@.str.0 = private unnamed_addr constant [9 x i8] c"value = \00", align 1
@.str.1 = private unnamed_addr constant [11 x i8] c", deriv = \00", align 1
@.str.2 = private unnamed_addr constant [48 x i8] c"=== Forward-Mode Autodiff with Dual Numbers ===\00", align 1
@.str.3 = private unnamed_addr constant [32 x i8] c"f(x) = x^3 + x^2 + x, at x = 2:\00", align 1
@.str.4 = private unnamed_addr constant [3 x i8] c"  \00", align 1
@.str.5 = private unnamed_addr constant [35 x i8] c"  Expected: value = 14, deriv = 17\00", align 1
@.str.6 = private unnamed_addr constant [1 x i8] c"\00", align 1
@.str.7 = private unnamed_addr constant [32 x i8] c"g(x) = 3x^2 - 2x + 5, at x = 3:\00", align 1
@.str.8 = private unnamed_addr constant [35 x i8] c"  Expected: value = 26, deriv = 16\00", align 1
@.str.9 = private unnamed_addr constant [30 x i8] c"h(x) = (x+1)*(x-1), at x = 4:\00", align 1
@.str.10 = private unnamed_addr constant [34 x i8] c"  Expected: value = 15, deriv = 8\00", align 1

declare void @iris_print(ptr)
declare void @iris_print_i64(i64)
declare void @iris_print_i32(i32)
declare void @iris_print_f64(double)
declare void @iris_print_f32(float)
declare void @iris_print_bool(i1)
declare void @iris_print_str(ptr)
declare void @iris_panic(ptr)
declare ptr @iris_read_line()
declare i64 @iris_read_i64()
declare double @iris_read_f64()
declare i64 @iris_str_len(ptr)
declare ptr @iris_str_concat(ptr, ptr)
declare i1 @iris_str_eq(ptr, ptr)
declare i1 @iris_str_contains(ptr, ptr)
declare i1 @iris_str_starts_with(ptr, ptr)
declare i1 @iris_str_ends_with(ptr, ptr)
declare ptr @iris_str_to_upper(ptr)
declare ptr @iris_str_to_lower(ptr)
declare ptr @iris_str_trim(ptr)
declare ptr @iris_str_repeat(ptr, i64)
declare ptr @iris_value_to_str(ptr)
declare ptr @iris_parse_i64(ptr)
declare ptr @iris_parse_f64(ptr)
declare i64 @iris_str_index(ptr, i64)
declare ptr @iris_str_slice(ptr, i64, i64)
declare ptr @iris_str_find(ptr, ptr)
declare ptr @iris_str_replace(ptr, ptr, ptr)
declare ptr @iris_const_str()
declare ptr @iris_make_some(ptr)
declare ptr @iris_make_none()
declare i1 @iris_is_some(ptr)
declare ptr @iris_option_unwrap(ptr)
declare ptr @iris_make_ok(ptr)
declare ptr @iris_make_err(ptr)
declare i1 @iris_is_ok(ptr)
declare ptr @iris_result_unwrap(ptr)
declare ptr @iris_result_unwrap_err(ptr)
declare ptr @iris_list_new()
declare void @iris_list_push(ptr, ptr)
declare i64 @iris_list_len(ptr)
declare ptr @iris_list_get(ptr, i64)
declare void @iris_list_set(ptr, i64, ptr)
declare ptr @iris_list_pop(ptr)
declare ptr @iris_map_new()
declare void @iris_map_set(ptr, ptr, ptr)
declare ptr @iris_map_get(ptr, ptr)
declare i1 @iris_map_contains(ptr, ptr)
declare void @iris_map_remove(ptr, ptr)
declare i64 @iris_map_len(ptr)
declare i1 @iris_list_contains(ptr, ptr)
declare void @iris_list_sort(ptr)
declare ptr @iris_map_keys(ptr)
declare ptr @iris_map_values(ptr)
declare ptr @iris_list_concat(ptr, ptr)
declare ptr @iris_list_slice(ptr, i64, i64)
declare ptr @iris_file_read_all(ptr)
declare ptr @iris_file_write_all(ptr, ptr)
declare i1 @iris_file_exists(ptr)
declare ptr @iris_file_lines(ptr)
declare i64 @iris_db_open(ptr)
declare i64 @iris_db_exec(i64, ptr)
declare ptr @iris_db_query(i64, ptr)
declare i64 @iris_db_close(i64)
declare void @exit(i32)
declare ptr @malloc(i64)
declare void @free(ptr)
declare void @iris_set_argv(i32, ptr)
declare ptr @iris_process_args()
declare ptr @iris_env_var(ptr)
declare ptr @iris_alloc_array()
declare ptr @iris_array_load(ptr, i64)
declare void @iris_array_store(ptr, i64, ptr)
declare ptr @iris_tensor_op()
declare ptr @iris_tensor_load(ptr, ...)
declare void @iris_tensor_store(ptr, ...)
declare ptr @iris_tensor_matmul(ptr, ptr)
declare ptr @iris_tensor_add(ptr, ptr)
declare ptr @iris_tensor_sub(ptr, ptr)
declare ptr @iris_tensor_mul(ptr, ptr)
declare ptr @iris_tensor_div(ptr, ptr)
declare ptr @iris_tensor_neg(ptr)
declare ptr @iris_tensor_relu(ptr)
declare ptr @iris_tensor_sigmoid(ptr)
declare ptr @iris_tensor_tanh_act(ptr)
declare ptr @iris_tensor_exp(ptr)
declare ptr @iris_tensor_log(ptr)
declare ptr @iris_tensor_sqrt(ptr)
declare ptr @iris_tensor_abs(ptr)
declare ptr @iris_tensor_reshape(ptr, i32, ptr)
declare ptr @iris_tensor_transpose(ptr, i32, ptr)
declare ptr @iris_tensor_reduce_sum(ptr, i32, i32)
declare ptr @iris_tensor_reduce_max(ptr, i32, i32)
declare ptr @iris_tensor_reduce_mean(ptr, i32, i32)
declare void @iris_retain(ptr)
declare void @iris_release(ptr)
declare void @iris_retain_kind(ptr, i32)
declare void @iris_release_kind(ptr, i32)
declare ptr @iris_chan_new()
declare void @iris_chan_send(ptr, ptr)
declare ptr @iris_chan_recv(ptr)
declare void @iris_spawn_fn(ptr, ptr)
declare void @iris_par_for(ptr, i64, i64)
declare void @iris_barrier()
declare ptr @iris_make_struct(i32, ...)
declare ptr @iris_get_field(ptr, i32)
declare ptr @iris_make_tuple(i32, ...)
declare ptr @iris_get_element(ptr, i32)
declare ptr @iris_make_closure(ptr, i32, ...)
declare ptr @iris_call_closure(ptr, ...)
declare void @iris_call_closure_void(ptr, ...)
declare ptr @iris_closure_fn(ptr)
declare ptr @iris_closure_get_capture(ptr, i32)
declare ptr @iris_atomic_new(ptr)
declare ptr @iris_atomic_load(ptr)
declare void @iris_atomic_store(ptr, ptr)
declare ptr @iris_atomic_add(ptr, ptr)
declare ptr @iris_mutex_new()
declare ptr @iris_mutex_lock(ptr)
declare void @iris_mutex_unlock(ptr)
declare ptr @iris_make_grad(double, double)
declare double @iris_grad_value(ptr)
declare double @iris_grad_tangent(ptr)
declare ptr @iris_tape_record(double, ptr, i64, ptr, ptr)
declare void @iris_backward(ptr)
declare double @iris_tape_grad(ptr)
declare ptr @iris_sparsify(ptr)
declare ptr @iris_densify(ptr)
declare ptr @iris_box_i64(i64)
declare ptr @iris_box_i32(i32)
declare ptr @iris_box_f64(double)
declare ptr @iris_box_f32(float)
declare ptr @iris_box_bool(i1)
declare ptr @iris_box_str(ptr)
declare ptr @iris_box_list(ptr)
declare ptr @iris_box_map(ptr)
declare ptr @iris_box_option(ptr)
declare ptr @iris_box_result(ptr)
declare ptr @iris_box_chan(ptr)
declare ptr @iris_box_atomic(ptr)
declare ptr @iris_box_mutex(ptr)
declare ptr @iris_box_grad(ptr)
declare ptr @iris_box_sparse(ptr)
declare i64 @iris_unbox_i64(ptr)
declare double @iris_unbox_f64(ptr)
declare i32 @iris_unbox_bool(ptr)
declare ptr @iris_unbox_str(ptr)
declare ptr @iris_unbox_list(ptr)
declare ptr @iris_unbox_map(ptr)
declare ptr @iris_unbox_option(ptr)
declare ptr @iris_unbox_result(ptr)
declare ptr @iris_unbox_chan(ptr)
declare ptr @iris_unbox_atomic(ptr)
declare ptr @iris_unbox_mutex(ptr)
declare ptr @iris_unbox_grad(ptr)
declare ptr @iris_unbox_sparse(ptr)
declare ptr @iris_i64_to_str(i64)
declare ptr @iris_i32_to_str(i32)
declare ptr @iris_f64_to_str(double)
declare ptr @iris_f32_to_str(float)
declare ptr @iris_bool_to_str(i1)
declare ptr @iris_str_to_str(ptr)
declare i64 @iris_pow_i64(i64, i64)
declare i64 @iris_min_i64(i64, i64)
declare i64 @iris_max_i64(i64, i64)
declare i64 @iris_abs_i64(i64)
declare double @iris_sign_f64(double)
declare double @tan(double)
declare double @llvm.sqrt.f64(double)
declare double @llvm.fabs.f64(double)
declare double @llvm.floor.f64(double)
declare double @llvm.ceil.f64(double)
declare double @llvm.round.f64(double)
declare double @llvm.sin.f64(double)
declare double @llvm.cos.f64(double)
declare double @llvm.exp.f64(double)
declare double @llvm.log.f64(double)
declare double @llvm.log2.f64(double)
declare double @llvm.pow.f64(double, double)
declare double @llvm.minnum.f64(double, double)
declare double @llvm.maxnum.f64(double, double)
declare ptr @iris_str_split(ptr, ptr)
declare ptr @iris_str_join(ptr, ptr)
declare i64 @iris_now_ms()
declare void @iris_sleep_ms(i64)
declare i64 @iris_tcp_connect(ptr, i64)
declare i64 @iris_tcp_listen(i64)
declare i64 @iris_tcp_accept(i64)
declare ptr @iris_tcp_read(i64)
declare void @iris_tcp_write(i64, ptr)
declare void @iris_tcp_close(i64)
declare i64 @iris_udp_open(i64)
declare void @iris_udp_send(i64, ptr, i64)
declare ptr @iris_udp_recv(i64)
declare void @iris_udp_close(i64)
declare ptr @iris_http_get(ptr)
declare ptr @iris_http_post(ptr, ptr, ptr)
declare ptr @iris_http_post_json(ptr, ptr)
declare ptr @iris_http_request(ptr, ptr, ptr, ptr)
declare ptr @iris_json_parse(ptr)
declare ptr @iris_json_stringify(ptr)
declare ptr @iris_set_new()
declare ptr @iris_set_add(ptr, ptr)
declare i1 @iris_set_contains(ptr, ptr)
declare ptr @iris_set_remove(ptr, ptr)
declare i64 @iris_set_len(ptr)
declare ptr @iris_set_to_list(ptr)
declare i1 @iris_regex_match(ptr, ptr)
declare ptr @iris_regex_find_all(ptr, ptr)
declare ptr @iris_regex_replace(ptr, ptr, ptr)
declare ptr @iris_datetime_now()
declare double @iris_datetime_timestamp()
declare ptr @iris_datetime_format(ptr)
declare ptr @iris_cwd()
declare ptr @iris_listdir(ptr)
declare ptr @iris_path_join(ptr, ptr)
declare i1 @iris_path_exists(ptr)
declare i1 @iris_mkdir(ptr)
declare i1 @iris_remove_file(ptr)
declare ptr @iris_type_of(ptr)
declare double @iris_random()
declare i64 @iris_random_range(i64, i64)
declare i64 @iris_hash(ptr)
declare ptr @iris_base64_encode(ptr)
declare ptr @iris_base64_decode(ptr)
declare ptr @iris_char_at(ptr, i64)
declare ptr @iris_str_reverse(ptr)
declare ptr @iris_str_pad_left(ptr, i64, ptr)
declare ptr @iris_str_pad_right(ptr, i64, ptr)
declare ptr @iris_str_chars(ptr)
declare ptr @iris_str_bytes(ptr)
declare i64 @iris_str_count(ptr, ptr)
declare double @iris_math_pi()
declare double @iris_math_e()
declare double @iris_math_inf()
declare i1 @iris_is_nan(double)
declare i1 @iris_is_inf(double)
declare ptr @iris_env_get(ptr)
declare void @iris_env_set(ptr, ptr)
declare void @iris_exit_code(i64)
declare ptr @iris_exec_cmd(ptr)
declare i64 @iris_pid()
declare ptr @iris_uuid()
declare ptr @iris_sha256(ptr)
declare ptr @iris_hex_encode(ptr)
declare ptr @iris_hex_decode(ptr)
declare ptr @iris_deque_new()
declare void @iris_deque_push_front(ptr, ptr)
declare void @iris_deque_push_back(ptr, ptr)
declare ptr @iris_deque_pop_front(ptr)
declare ptr @iris_deque_pop_back(ptr)
declare i64 @iris_deque_len(ptr)
declare ptr @iris_ffi_open(ptr)
declare i64 @iris_ffi_call(ptr, ptr)
declare i1 @iris_ffi_close(ptr)
declare i64 @iris_ffi_call_i64(ptr, ptr, ptr, i32)
declare double @iris_ffi_call_f64(ptr, ptr, ptr, i32)
declare ptr @iris_ffi_call_str(ptr, ptr, ptr, i32)
declare void @iris_ffi_call_void(ptr, ptr, ptr, i32)
declare ptr @iris_python_eval(ptr)
declare i64 @iris_python_exec(ptr)
declare ptr @iris_python_call(ptr, ptr, ptr)
declare ptr @iris_python_version()
declare ptr @iris_rust_lib_open(ptr)
declare i64 @iris_rust_call_i64(ptr, ptr, ptr, i32)
declare double @iris_rust_call_f64(ptr, ptr, ptr, i32)
declare void @iris_rust_call_void(ptr, ptr, ptr, i32)
declare double @iris_list_sum(ptr)
declare i64 @iris_list_min(ptr)
declare i64 @iris_list_max(ptr)
declare i64 @iris_list_index_of(ptr, i64)
declare i64 @iris_list_count(ptr, i64)
declare ptr @iris_list_reverse(ptr)
declare ptr @iris_list_take(ptr, i64)
declare ptr @iris_list_drop(ptr, i64)
declare ptr @iris_deque_front(ptr)
declare ptr @iris_deque_back(ptr)
declare ptr @iris_chan_try_recv(ptr)
declare i64 @iris_chan_len(ptr)
declare i64 @iris_select(ptr, ...)
declare i1 @iris_timeout(i64)
declare i64 @iris_ffi_call_args(ptr, ptr, ptr, i32)
declare i64 @iris_thread_count()
declare i64 @iris_read_key()
declare ptr @iris_read_password(ptr)
declare void @iris_term_clear()
declare void @iris_term_cursor(i64, i64)
declare void @iris_term_show_cursor(i32)
declare void @iris_term_set_color(i64, i64)
declare void @iris_term_reset()
declare i64 @iris_term_rows()
declare i64 @iris_term_cols()

define ptr @dual_var(i64 %x) nounwind willreturn {
entry0:
  %struct_sz2 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes2 = ptrtoint ptr %struct_sz2 to i64
  %struct_alloc2 = call ptr @malloc(i64 %struct_bytes2)
  %sgep2_0 = getelementptr inbounds %Dual, ptr %struct_alloc2, i32 0, i32 0
  store i64 %x, ptr %sgep2_0, align 8
  %sgep2_1 = getelementptr inbounds %Dual, ptr %struct_alloc2, i32 0, i32 1
  store i64 1, ptr %sgep2_1, align 8
  %v2 = getelementptr inbounds %Dual, ptr %struct_alloc2, i32 0
  ret ptr %v2
}

define ptr @dual_const(i64 %c) nounwind willreturn {
entry0:
  %struct_sz2 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes2 = ptrtoint ptr %struct_sz2 to i64
  %struct_alloc2 = call ptr @malloc(i64 %struct_bytes2)
  %sgep2_0 = getelementptr inbounds %Dual, ptr %struct_alloc2, i32 0, i32 0
  store i64 %c, ptr %sgep2_0, align 8
  %sgep2_1 = getelementptr inbounds %Dual, ptr %struct_alloc2, i32 0, i32 1
  store i64 0, ptr %sgep2_1, align 8
  %v2 = getelementptr inbounds %Dual, ptr %struct_alloc2, i32 0
  ret ptr %v2
}

define ptr @dual_add(ptr %a, ptr %b) nounwind willreturn {
entry0:
  %fgep2_0 = getelementptr inbounds %Dual, ptr %a, i32 0, i32 0
  %v2 = load i64, ptr %fgep2_0, align 8
  %fgep3_0 = getelementptr inbounds %Dual, ptr %b, i32 0, i32 0
  %v3 = load i64, ptr %fgep3_0, align 8
  %v4 = add nsw i64 %v2, %v3
  %fgep5_1 = getelementptr inbounds %Dual, ptr %a, i32 0, i32 1
  %v5 = load i64, ptr %fgep5_1, align 8
  %fgep6_1 = getelementptr inbounds %Dual, ptr %b, i32 0, i32 1
  %v6 = load i64, ptr %fgep6_1, align 8
  %v7 = add nsw i64 %v5, %v6
  %struct_sz8 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes8 = ptrtoint ptr %struct_sz8 to i64
  %struct_alloc8 = call ptr @malloc(i64 %struct_bytes8)
  %sgep8_0 = getelementptr inbounds %Dual, ptr %struct_alloc8, i32 0, i32 0
  store i64 %v4, ptr %sgep8_0, align 8
  %sgep8_1 = getelementptr inbounds %Dual, ptr %struct_alloc8, i32 0, i32 1
  store i64 %v7, ptr %sgep8_1, align 8
  %v8 = getelementptr inbounds %Dual, ptr %struct_alloc8, i32 0
  ret ptr %v8
}

define ptr @dual_mul(ptr %a, ptr %b) nounwind willreturn {
entry0:
  %fgep2_0 = getelementptr inbounds %Dual, ptr %a, i32 0, i32 0
  %v2 = load i64, ptr %fgep2_0, align 8
  %fgep3_0 = getelementptr inbounds %Dual, ptr %b, i32 0, i32 0
  %v3 = load i64, ptr %fgep3_0, align 8
  %v4 = mul nsw i64 %v2, %v3
  %fgep5_1 = getelementptr inbounds %Dual, ptr %a, i32 0, i32 1
  %v5 = load i64, ptr %fgep5_1, align 8
  %fgep6_0 = getelementptr inbounds %Dual, ptr %b, i32 0, i32 0
  %v6 = load i64, ptr %fgep6_0, align 8
  %v7 = mul nsw i64 %v5, %v6
  %fgep8_0 = getelementptr inbounds %Dual, ptr %a, i32 0, i32 0
  %v8 = load i64, ptr %fgep8_0, align 8
  %fgep9_1 = getelementptr inbounds %Dual, ptr %b, i32 0, i32 1
  %v9 = load i64, ptr %fgep9_1, align 8
  %v10 = mul nsw i64 %v8, %v9
  %v11 = add nsw i64 %v7, %v10
  %struct_sz12 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes12 = ptrtoint ptr %struct_sz12 to i64
  %struct_alloc12 = call ptr @malloc(i64 %struct_bytes12)
  %sgep12_0 = getelementptr inbounds %Dual, ptr %struct_alloc12, i32 0, i32 0
  store i64 %v4, ptr %sgep12_0, align 8
  %sgep12_1 = getelementptr inbounds %Dual, ptr %struct_alloc12, i32 0, i32 1
  store i64 %v11, ptr %sgep12_1, align 8
  %v12 = getelementptr inbounds %Dual, ptr %struct_alloc12, i32 0
  ret ptr %v12
}

define ptr @dual_sub(ptr %a, ptr %b) nounwind willreturn {
entry0:
  %fgep2_0 = getelementptr inbounds %Dual, ptr %a, i32 0, i32 0
  %v2 = load i64, ptr %fgep2_0, align 8
  %fgep3_0 = getelementptr inbounds %Dual, ptr %b, i32 0, i32 0
  %v3 = load i64, ptr %fgep3_0, align 8
  %v4 = sub nsw i64 %v2, %v3
  %fgep5_1 = getelementptr inbounds %Dual, ptr %a, i32 0, i32 1
  %v5 = load i64, ptr %fgep5_1, align 8
  %fgep6_1 = getelementptr inbounds %Dual, ptr %b, i32 0, i32 1
  %v6 = load i64, ptr %fgep6_1, align 8
  %v7 = sub nsw i64 %v5, %v6
  %struct_sz8 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes8 = ptrtoint ptr %struct_sz8 to i64
  %struct_alloc8 = call ptr @malloc(i64 %struct_bytes8)
  %sgep8_0 = getelementptr inbounds %Dual, ptr %struct_alloc8, i32 0, i32 0
  store i64 %v4, ptr %sgep8_0, align 8
  %sgep8_1 = getelementptr inbounds %Dual, ptr %struct_alloc8, i32 0, i32 1
  store i64 %v7, ptr %sgep8_1, align 8
  %v8 = getelementptr inbounds %Dual, ptr %struct_alloc8, i32 0
  ret ptr %v8
}

define i64 @show_dual(ptr %label, ptr %d) {
entry0:
  %v2 = getelementptr inbounds [9 x i8], ptr @.str.0, i32 0, i32 0
  %fgep3_0 = getelementptr inbounds %Dual, ptr %d, i32 0, i32 0
  %v3 = load i64, ptr %fgep3_0, align 8
  %v4 = call ptr @iris_i64_to_str(i64 %v3)
  %v5 = getelementptr inbounds [11 x i8], ptr @.str.1, i32 0, i32 0
  %fgep6_1 = getelementptr inbounds %Dual, ptr %d, i32 0, i32 1
  %v6 = load i64, ptr %fgep6_1, align 8
  %v7 = call ptr @iris_i64_to_str(i64 %v6)
  %v8 = call ptr @iris_str_concat(ptr %v5, ptr %v7)
  %v9 = call ptr @iris_str_concat(ptr %v4, ptr %v8)
  %v10 = call ptr @iris_str_concat(ptr %v2, ptr %v9)
  %v11 = call ptr @iris_str_concat(ptr %label, ptr %v10)
  call void @iris_print_str(ptr %v11)
  ret i64 0
}

define i64 @main() {
entry0:
  %v0 = getelementptr inbounds [48 x i8], ptr @.str.2, i32 0, i32 0
  call void @iris_print_str(ptr %v0)
  %struct_sz58 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes58 = ptrtoint ptr %struct_sz58 to i64
  %struct_alloc58 = call ptr @malloc(i64 %struct_bytes58)
  %sgep58_0 = getelementptr inbounds %Dual, ptr %struct_alloc58, i32 0, i32 0
  store i64 2, ptr %sgep58_0, align 8
  %sgep58_1 = getelementptr inbounds %Dual, ptr %struct_alloc58, i32 0, i32 1
  store i64 1, ptr %sgep58_1, align 8
  %v58 = getelementptr inbounds %Dual, ptr %struct_alloc58, i32 0
  %v4 = call ptr @dual_mul(ptr %v58, ptr %v58)
  %v5 = call ptr @dual_mul(ptr %v4, ptr %v58)
  %fgep59_0 = getelementptr inbounds %Dual, ptr %v5, i32 0, i32 0
  %v59 = load i64, ptr %fgep59_0, align 8
  %fgep60_0 = getelementptr inbounds %Dual, ptr %v4, i32 0, i32 0
  %v60 = load i64, ptr %fgep60_0, align 8
  %v61 = add nsw i64 %v59, %v60
  %fgep62_1 = getelementptr inbounds %Dual, ptr %v5, i32 0, i32 1
  %v62 = load i64, ptr %fgep62_1, align 8
  %fgep63_1 = getelementptr inbounds %Dual, ptr %v4, i32 0, i32 1
  %v63 = load i64, ptr %fgep63_1, align 8
  %v64 = add nsw i64 %v62, %v63
  %struct_sz65 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes65 = ptrtoint ptr %struct_sz65 to i64
  %struct_alloc65 = call ptr @malloc(i64 %struct_bytes65)
  %sgep65_0 = getelementptr inbounds %Dual, ptr %struct_alloc65, i32 0, i32 0
  store i64 %v61, ptr %sgep65_0, align 8
  %sgep65_1 = getelementptr inbounds %Dual, ptr %struct_alloc65, i32 0, i32 1
  store i64 %v64, ptr %sgep65_1, align 8
  %v65 = getelementptr inbounds %Dual, ptr %struct_alloc65, i32 0
  %fgep66_0 = getelementptr inbounds %Dual, ptr %v65, i32 0, i32 0
  %v66 = load i64, ptr %fgep66_0, align 8
  %fgep67_0 = getelementptr inbounds %Dual, ptr %v58, i32 0, i32 0
  %v67 = load i64, ptr %fgep67_0, align 8
  %v68 = add nsw i64 %v66, %v67
  %fgep69_1 = getelementptr inbounds %Dual, ptr %v65, i32 0, i32 1
  %v69 = load i64, ptr %fgep69_1, align 8
  %fgep70_1 = getelementptr inbounds %Dual, ptr %v58, i32 0, i32 1
  %v70 = load i64, ptr %fgep70_1, align 8
  %v71 = add nsw i64 %v69, %v70
  %struct_sz72 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes72 = ptrtoint ptr %struct_sz72 to i64
  %struct_alloc72 = call ptr @malloc(i64 %struct_bytes72)
  %sgep72_0 = getelementptr inbounds %Dual, ptr %struct_alloc72, i32 0, i32 0
  store i64 %v68, ptr %sgep72_0, align 8
  %sgep72_1 = getelementptr inbounds %Dual, ptr %struct_alloc72, i32 0, i32 1
  store i64 %v71, ptr %sgep72_1, align 8
  %v72 = getelementptr inbounds %Dual, ptr %struct_alloc72, i32 0
  %v8 = getelementptr inbounds [32 x i8], ptr @.str.3, i32 0, i32 0
  call void @iris_print_str(ptr %v8)
  %v10 = getelementptr inbounds [3 x i8], ptr @.str.4, i32 0, i32 0
  %v11 = call i64 @show_dual(ptr %v10, ptr %v72)
  %v12 = getelementptr inbounds [35 x i8], ptr @.str.5, i32 0, i32 0
  %v13 = getelementptr inbounds [1 x i8], ptr @.str.6, i32 0, i32 0
  %v14 = call ptr @iris_str_concat(ptr %v12, ptr %v13)
  call void @iris_print_str(ptr %v14)
  %v16 = getelementptr inbounds [1 x i8], ptr @.str.6, i32 0, i32 0
  call void @iris_print_str(ptr %v16)
  %struct_sz74 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes74 = ptrtoint ptr %struct_sz74 to i64
  %struct_alloc74 = call ptr @malloc(i64 %struct_bytes74)
  %sgep74_0 = getelementptr inbounds %Dual, ptr %struct_alloc74, i32 0, i32 0
  store i64 3, ptr %sgep74_0, align 8
  %sgep74_1 = getelementptr inbounds %Dual, ptr %struct_alloc74, i32 0, i32 1
  store i64 1, ptr %sgep74_1, align 8
  %v74 = getelementptr inbounds %Dual, ptr %struct_alloc74, i32 0
  %v20 = call ptr @dual_mul(ptr %v74, ptr %v74)
  %struct_sz76 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes76 = ptrtoint ptr %struct_sz76 to i64
  %struct_alloc76 = call ptr @malloc(i64 %struct_bytes76)
  %sgep76_0 = getelementptr inbounds %Dual, ptr %struct_alloc76, i32 0, i32 0
  store i64 3, ptr %sgep76_0, align 8
  %sgep76_1 = getelementptr inbounds %Dual, ptr %struct_alloc76, i32 0, i32 1
  store i64 0, ptr %sgep76_1, align 8
  %v76 = getelementptr inbounds %Dual, ptr %struct_alloc76, i32 0
  %struct_sz78 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes78 = ptrtoint ptr %struct_sz78 to i64
  %struct_alloc78 = call ptr @malloc(i64 %struct_bytes78)
  %sgep78_0 = getelementptr inbounds %Dual, ptr %struct_alloc78, i32 0, i32 0
  store i64 2, ptr %sgep78_0, align 8
  %sgep78_1 = getelementptr inbounds %Dual, ptr %struct_alloc78, i32 0, i32 1
  store i64 0, ptr %sgep78_1, align 8
  %v78 = getelementptr inbounds %Dual, ptr %struct_alloc78, i32 0
  %struct_sz80 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes80 = ptrtoint ptr %struct_sz80 to i64
  %struct_alloc80 = call ptr @malloc(i64 %struct_bytes80)
  %sgep80_0 = getelementptr inbounds %Dual, ptr %struct_alloc80, i32 0, i32 0
  store i64 5, ptr %sgep80_0, align 8
  %sgep80_1 = getelementptr inbounds %Dual, ptr %struct_alloc80, i32 0, i32 1
  store i64 0, ptr %sgep80_1, align 8
  %v80 = getelementptr inbounds %Dual, ptr %struct_alloc80, i32 0
  %v27 = call ptr @dual_mul(ptr %v76, ptr %v20)
  %v28 = call ptr @dual_mul(ptr %v78, ptr %v74)
  %fgep81_0 = getelementptr inbounds %Dual, ptr %v27, i32 0, i32 0
  %v81 = load i64, ptr %fgep81_0, align 8
  %fgep82_0 = getelementptr inbounds %Dual, ptr %v28, i32 0, i32 0
  %v82 = load i64, ptr %fgep82_0, align 8
  %v83 = sub nsw i64 %v81, %v82
  %fgep84_1 = getelementptr inbounds %Dual, ptr %v27, i32 0, i32 1
  %v84 = load i64, ptr %fgep84_1, align 8
  %fgep85_1 = getelementptr inbounds %Dual, ptr %v28, i32 0, i32 1
  %v85 = load i64, ptr %fgep85_1, align 8
  %v86 = sub nsw i64 %v84, %v85
  %struct_sz87 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes87 = ptrtoint ptr %struct_sz87 to i64
  %struct_alloc87 = call ptr @malloc(i64 %struct_bytes87)
  %sgep87_0 = getelementptr inbounds %Dual, ptr %struct_alloc87, i32 0, i32 0
  store i64 %v83, ptr %sgep87_0, align 8
  %sgep87_1 = getelementptr inbounds %Dual, ptr %struct_alloc87, i32 0, i32 1
  store i64 %v86, ptr %sgep87_1, align 8
  %v87 = getelementptr inbounds %Dual, ptr %struct_alloc87, i32 0
  %fgep88_0 = getelementptr inbounds %Dual, ptr %v87, i32 0, i32 0
  %v88 = load i64, ptr %fgep88_0, align 8
  %fgep89_0 = getelementptr inbounds %Dual, ptr %v80, i32 0, i32 0
  %v89 = load i64, ptr %fgep89_0, align 8
  %v90 = add nsw i64 %v88, %v89
  %fgep91_1 = getelementptr inbounds %Dual, ptr %v87, i32 0, i32 1
  %v91 = load i64, ptr %fgep91_1, align 8
  %fgep92_1 = getelementptr inbounds %Dual, ptr %v80, i32 0, i32 1
  %v92 = load i64, ptr %fgep92_1, align 8
  %v93 = add nsw i64 %v91, %v92
  %struct_sz94 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes94 = ptrtoint ptr %struct_sz94 to i64
  %struct_alloc94 = call ptr @malloc(i64 %struct_bytes94)
  %sgep94_0 = getelementptr inbounds %Dual, ptr %struct_alloc94, i32 0, i32 0
  store i64 %v90, ptr %sgep94_0, align 8
  %sgep94_1 = getelementptr inbounds %Dual, ptr %struct_alloc94, i32 0, i32 1
  store i64 %v93, ptr %sgep94_1, align 8
  %v94 = getelementptr inbounds %Dual, ptr %struct_alloc94, i32 0
  %v31 = getelementptr inbounds [32 x i8], ptr @.str.7, i32 0, i32 0
  call void @iris_print_str(ptr %v31)
  %v33 = getelementptr inbounds [3 x i8], ptr @.str.4, i32 0, i32 0
  %v34 = call i64 @show_dual(ptr %v33, ptr %v94)
  %v35 = getelementptr inbounds [35 x i8], ptr @.str.8, i32 0, i32 0
  %v36 = getelementptr inbounds [1 x i8], ptr @.str.6, i32 0, i32 0
  %v37 = call ptr @iris_str_concat(ptr %v35, ptr %v36)
  call void @iris_print_str(ptr %v37)
  %v39 = getelementptr inbounds [1 x i8], ptr @.str.6, i32 0, i32 0
  call void @iris_print_str(ptr %v39)
  %struct_sz96 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes96 = ptrtoint ptr %struct_sz96 to i64
  %struct_alloc96 = call ptr @malloc(i64 %struct_bytes96)
  %sgep96_0 = getelementptr inbounds %Dual, ptr %struct_alloc96, i32 0, i32 0
  store i64 4, ptr %sgep96_0, align 8
  %sgep96_1 = getelementptr inbounds %Dual, ptr %struct_alloc96, i32 0, i32 1
  store i64 1, ptr %sgep96_1, align 8
  %v96 = getelementptr inbounds %Dual, ptr %struct_alloc96, i32 0
  %struct_sz98 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes98 = ptrtoint ptr %struct_sz98 to i64
  %struct_alloc98 = call ptr @malloc(i64 %struct_bytes98)
  %sgep98_0 = getelementptr inbounds %Dual, ptr %struct_alloc98, i32 0, i32 0
  store i64 1, ptr %sgep98_0, align 8
  %sgep98_1 = getelementptr inbounds %Dual, ptr %struct_alloc98, i32 0, i32 1
  store i64 0, ptr %sgep98_1, align 8
  %v98 = getelementptr inbounds %Dual, ptr %struct_alloc98, i32 0
  %fgep99_0 = getelementptr inbounds %Dual, ptr %v96, i32 0, i32 0
  %v99 = load i64, ptr %fgep99_0, align 8
  %fgep100_0 = getelementptr inbounds %Dual, ptr %v98, i32 0, i32 0
  %v100 = load i64, ptr %fgep100_0, align 8
  %v101 = add nsw i64 %v99, %v100
  %fgep102_1 = getelementptr inbounds %Dual, ptr %v96, i32 0, i32 1
  %v102 = load i64, ptr %fgep102_1, align 8
  %fgep103_1 = getelementptr inbounds %Dual, ptr %v98, i32 0, i32 1
  %v103 = load i64, ptr %fgep103_1, align 8
  %v104 = add nsw i64 %v102, %v103
  %struct_sz105 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes105 = ptrtoint ptr %struct_sz105 to i64
  %struct_alloc105 = call ptr @malloc(i64 %struct_bytes105)
  %sgep105_0 = getelementptr inbounds %Dual, ptr %struct_alloc105, i32 0, i32 0
  store i64 %v101, ptr %sgep105_0, align 8
  %sgep105_1 = getelementptr inbounds %Dual, ptr %struct_alloc105, i32 0, i32 1
  store i64 %v104, ptr %sgep105_1, align 8
  %v105 = getelementptr inbounds %Dual, ptr %struct_alloc105, i32 0
  %fgep106_0 = getelementptr inbounds %Dual, ptr %v96, i32 0, i32 0
  %v106 = load i64, ptr %fgep106_0, align 8
  %fgep107_0 = getelementptr inbounds %Dual, ptr %v98, i32 0, i32 0
  %v107 = load i64, ptr %fgep107_0, align 8
  %v108 = sub nsw i64 %v106, %v107
  %fgep109_1 = getelementptr inbounds %Dual, ptr %v96, i32 0, i32 1
  %v109 = load i64, ptr %fgep109_1, align 8
  %fgep110_1 = getelementptr inbounds %Dual, ptr %v98, i32 0, i32 1
  %v110 = load i64, ptr %fgep110_1, align 8
  %v111 = sub nsw i64 %v109, %v110
  %struct_sz112 = getelementptr %Dual, ptr null, i32 1
  %struct_bytes112 = ptrtoint ptr %struct_sz112 to i64
  %struct_alloc112 = call ptr @malloc(i64 %struct_bytes112)
  %sgep112_0 = getelementptr inbounds %Dual, ptr %struct_alloc112, i32 0, i32 0
  store i64 %v108, ptr %sgep112_0, align 8
  %sgep112_1 = getelementptr inbounds %Dual, ptr %struct_alloc112, i32 0, i32 1
  store i64 %v111, ptr %sgep112_1, align 8
  %v112 = getelementptr inbounds %Dual, ptr %struct_alloc112, i32 0
  %v47 = call ptr @dual_mul(ptr %v105, ptr %v112)
  %v48 = getelementptr inbounds [30 x i8], ptr @.str.9, i32 0, i32 0
  call void @iris_print_str(ptr %v48)
  %v50 = getelementptr inbounds [3 x i8], ptr @.str.4, i32 0, i32 0
  %v51 = call i64 @show_dual(ptr %v50, ptr %v47)
  %v52 = getelementptr inbounds [34 x i8], ptr @.str.10, i32 0, i32 0
  %v53 = getelementptr inbounds [1 x i8], ptr @.str.6, i32 0, i32 0
  %v54 = call ptr @iris_str_concat(ptr %v52, ptr %v53)
  call void @iris_print_str(ptr %v54)
  ret i64 0
}

