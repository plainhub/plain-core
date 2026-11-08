use super::*;

#[test]
fn merged_subscriptions_register_members_without_intermediate_types() {
    let input = syn::parse_quote!(
        struct Events(Left, Right);
    );
    let args = args::MergedSubscription::from_derive_input(&input).unwrap();
    let generated = generate(&args).unwrap();
    syn::parse2::<syn::File>(generated.clone()).unwrap();
    let generated = generated.to_string();
    assert_eq!(
        generated.matches("create_fake_subscription_type").count(),
        2
    );
    assert_eq!(generated.matches("create_field_stream (& self").count(), 2);
    assert!(!generated.contains("MergedObject"));
}
