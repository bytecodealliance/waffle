(module
  (type $t (func))
  (func (local funcref) (drop (local.get 0)))
  (func (result externref) (local externref) (local.get 0))
  (func (result (ref null $t)) (local (ref null $t)) (local.get 0)))
