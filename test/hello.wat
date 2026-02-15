;; Minimal: returns a fixed string "hi"
(module
  (memory (export "memory") 3)
  ;; Pre-store output at 0x20000: len=2, data="hi"
  (data (i32.const 0x20000) "\02\00\00\00hi")

  (func (export "run") (param $input_ptr i32) (param $input_len i32) (result i32)
    (i32.const 0x20000)
  )
)
