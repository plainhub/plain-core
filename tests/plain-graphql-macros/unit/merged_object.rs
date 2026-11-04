use super::*;
use darling::FromDeriveInput;
use syn::visit::{self, Visit};

#[derive(Default)]
struct Registration {
    members: Vec<String>,
    merge_types: usize,
}

impl<'ast> Visit<'ast> for Registration {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "create_fake_output_type" {
            self.members.push(quote!(#node).to_string());
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        self.merge_types +=
            usize::from(node.path.segments.iter().any(|s| s.ident == "MergedObject"));
        visit::visit_type_path(self, node);
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
        let generated = syn::parse2::<syn::File>(generate(&args).unwrap()).unwrap();
        let mut registration = Registration::default();
        registration.visit_file(&generated);
        assert_eq!(registration.members.len(), count);
        assert_eq!(registration.merge_types, 0);
        for (call, member) in registration.members.iter().zip(types) {
            assert!(call.contains(&member.to_string()));
        }
    }
}
