{
  let map2 = fun f l1 l2 =>
    if l == [] || l2 == [] then
      []
    else
      [f (lists.head l1) (lists.head l2)]
      @ map2 f (lists.tail l1) (lists.tail l2),

  run = fun n =>
    let left = lists.generate builtins.id n in
    let right = lists.generate (fun n => n*n/2) in
    let prod = map2 (fun x y => x * y) left right in
    lists.foldl (fun x y => x + y) 0 prod
}