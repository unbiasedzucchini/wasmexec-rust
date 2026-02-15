;; Echo module: copies input to output.
(module
  (memory (export "memory") 3)

  (func (export "run") (param $input_ptr i32) (param $input_len i32) (result i32)
    (local $i i32)
    ;; Write output length at 0x20000
    (i32.store (i32.const 0x20000) (local.get $input_len))
    ;; Copy input to 0x20004 byte by byte
    (local.set $i (i32.const 0))
    (block $break
      (loop $loop
        (br_if $break (i32.ge_u (local.get $i) (local.get $input_len)))
        (i32.store8
          (i32.add (i32.const 0x20004) (local.get $i))
          (i32.load8_u (i32.add (local.get $input_ptr) (local.get $i)))
        )
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)
      )
    )
    (i32.const 0x20000)
  )
)
