;; Reverse module: reverses input bytes.
(module
  (memory (export "memory") 3)

  (func (export "run") (param $input_ptr i32) (param $input_len i32) (result i32)
    (local $i i32)
    (local $out_data i32)

    ;; out_data = 0x20004
    (local.set $out_data (i32.const 0x20004))

    ;; Write output length
    (i32.store (i32.const 0x20000) (local.get $input_len))

    ;; Reverse copy: out[i] = input[input_len - 1 - i]
    (local.set $i (i32.const 0))
    (block $break
      (loop $loop
        (br_if $break (i32.ge_u (local.get $i) (local.get $input_len)))
        (i32.store8
          (i32.add (local.get $out_data) (local.get $i))
          (i32.load8_u
            (i32.add
              (local.get $input_ptr)
              (i32.sub (i32.sub (local.get $input_len) (i32.const 1)) (local.get $i))
            )
          )
        )
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)
      )
    )

    (i32.const 0x20000)
  )
)
