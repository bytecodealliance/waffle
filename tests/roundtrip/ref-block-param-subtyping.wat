(module
  (func $f (export "f") (param funcref i32) (result i32)
        (if (local.get 1)
          (then (local.set 0 (ref.func $f))))
        (ref.is_null (local.get 0))))
