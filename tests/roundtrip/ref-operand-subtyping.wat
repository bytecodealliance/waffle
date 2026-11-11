(module
  (type $t (func))
  (table 1 funcref)
  (elem (i32.const 0) func $f)

  (func $f (type $t))

  (func $call-non-nullable (param (ref $t))
        (call_ref $t (local.get 0)))

  (func $funcref-operand
        (table.set (i32.const 0) (ref.func $f))))
