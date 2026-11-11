(module
  (table 1 funcref)
  (func (param funcref i32) (result i32)
        (i32.add (table.grow 0 (local.get 0) (local.get 1)) (i32.const 1))))
