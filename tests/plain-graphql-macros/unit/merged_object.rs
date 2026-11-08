use super::*;
use syn::visit_mut::{self, VisitMut};

#[derive(Default)]
struct Registration {
    members: Vec<String>,
    merge_types: usize,
}

impl VisitMut for Registration {
    fn visit_expr_method_call_mut(&mut self, node: &mut syn::ExprMethodCall) {
        if node.method == "create_fake_output_type" {
            self.members.push(quote!(#node).to_string());
        }
        visit_mut::visit_expr_method_call_mut(self, node);
    }

    fn visit_type_path_mut(&mut self, node: &mut syn::TypePath) {
        self.merge_types +=
            usize::from(node.path.segments.iter().any(|s| s.ident == "MergedObject"));
        visit_mut::visit_type_path_mut(self, node);
    }
}

#[test]
fn registration_calls_members_directly_without_intermediate_merge_types() {
    for count in [81, 8, 1, 0] {
        let types: Vec<_> = (0..count)
            .map(|i| quote::format_ident!("Member{i}"))
            .collect();
        let input = syn::parse2(quote!(struct Root(#(#types),*);)).unwrap();
        let args = args::MergedObject::from_derive_input(&input).unwrap();
        let mut generated = syn::parse2::<syn::File>(generate(&args).unwrap()).unwrap();
        let mut registration = Registration::default();
        registration.visit_file_mut(&mut generated);
        assert_eq!(registration.members.len(), count);
        assert_eq!(registration.merge_types, 0);
        for (call, member) in registration.members.iter().zip(types) {
            assert!(call.contains(&member.to_string()));
        }
    }
}
