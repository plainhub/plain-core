macro_rules! if_downcast_into {
    ($in_ty : ty , $out_ty : ty , $val : ident , $body : expr) => {{
        if std::any::TypeId::of::<$in_ty>() == std::any::TypeId::of::<$out_ty>() {
            let mut slot = Some($val);
            let $val = (&mut slot as &mut dyn std::any::Any)
                .downcast_mut::<Option<$out_ty>>()
                .unwrap()
                .take()
                .unwrap();
            $body
        }
    }};
}
