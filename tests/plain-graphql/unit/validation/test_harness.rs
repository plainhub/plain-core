use crate::{
    futures_util::stream::Stream,
    parser::types::ExecutableDocument,
    validation::visitor::{RuleError, Visitor, visit},
    *,
};
use std::sync::OnceLock;
#[derive(InputObject)]
#[graphql(internal)]
struct TestInput {
    id: i32,
    name: String,
}
impl Default for TestInput {
    fn default() -> Self {
        Self {
            id: 423,
            name: "foo".to_string(),
        }
    }
}
#[derive(Enum, Eq, PartialEq, Copy, Clone)]
#[graphql(internal)]
enum DogCommand {
    Sit,
    Heel,
    Down,
}
struct Dog;
#[Object(internal)]
impl Dog {
    async fn name(&self, surname: Option<bool>) -> Option<String> {
        validation_fixture((surname,))
    }
    async fn nickname(&self) -> Option<String> {
        validation_fixture(())
    }
    async fn bark_volume(&self) -> Option<i32> {
        validation_fixture(())
    }
    async fn barks(&self) -> Option<bool> {
        validation_fixture(())
    }
    async fn does_know_command(&self, dog_command: Option<DogCommand>) -> Option<bool> {
        validation_fixture((dog_command,))
    }
    async fn is_housetrained(
        &self,
        #[graphql(default = true)] at_other_homes: bool,
    ) -> Option<bool> {
        validation_fixture((at_other_homes,))
    }
    async fn is_at_location(&self, x: Option<i32>, y: Option<i32>) -> Option<bool> {
        validation_fixture((x, y))
    }
}
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(internal)]
enum FurColor {
    Brown,
    Black,
    Tan,
    Spotted,
}
struct Cat;
#[Object(internal)]
impl Cat {
    async fn name(&self, surname: Option<bool>) -> Option<String> {
        validation_fixture((surname,))
    }
    async fn nickname(&self) -> Option<String> {
        validation_fixture(())
    }
    async fn meows(&self) -> Option<bool> {
        validation_fixture(())
    }
    async fn meow_volume(&self) -> Option<i32> {
        validation_fixture(())
    }
    async fn fur_color(&self) -> Option<FurColor> {
        validation_fixture(())
    }
}
#[derive(Union)]
#[graphql(internal)]
enum CatOrDog {
    Cat(Cat),
    Dog(Dog),
}
struct Human;
#[Object(internal)]
impl Human {
    async fn name(&self, surname: Option<bool>) -> Option<String> {
        validation_fixture((surname,))
    }
    async fn pets(&self) -> Option<Vec<Option<Pet>>> {
        validation_fixture(())
    }
    async fn relatives(&self) -> Option<Vec<Human>> {
        validation_fixture(())
    }
    async fn iq(&self) -> Option<i32> {
        validation_fixture(())
    }
}
struct Alien;
#[Object(internal)]
impl Alien {
    async fn name(&self, surname: Option<bool>) -> Option<String> {
        validation_fixture((surname,))
    }
    async fn iq(&self) -> Option<i32> {
        validation_fixture(())
    }
    async fn num_eyes(&self) -> Option<i32> {
        validation_fixture(())
    }
}
#[derive(Union)]
#[graphql(internal)]
enum DogOrHuman {
    Dog(Dog),
    Human(Human),
}
#[derive(Union)]
#[graphql(internal)]
enum HumanOrAlien {
    Human(Human),
    Alien(Alien),
}
#[derive(Interface)]
#[graphql(
    internal,
    field(
        name = "name",
        ty = "Option<String>",
        arg(name = "surname", ty = "Option<bool>")
    )
)]
enum Being {
    Dog(Dog),
    Cat(Cat),
    Human(Human),
    Alien(Alien),
}
#[derive(Interface)]
#[graphql(
    internal,
    field(
        name = "name",
        ty = "Option<String>",
        arg(name = "surname", ty = "Option<bool>")
    )
)]
enum Pet {
    Dog(Dog),
    Cat(Cat),
}
#[derive(Interface)]
#[graphql(internal, field(name = "iq", ty = "Option<i32>"))]
enum Intelligent {
    Human(Human),
    Alien(Alien),
}
#[derive(InputObject)]
#[graphql(internal)]
struct ComplexInput {
    required_field: bool,
    int_field: Option<i32>,
    string_field: Option<String>,
    boolean_field: Option<bool>,
    string_list_field: Option<Vec<Option<String>>>,
}
struct ComplicatedArgs;
#[Object(internal)]
impl ComplicatedArgs {
    async fn int_arg_field(&self, int_arg: Option<i32>) -> Option<String> {
        validation_fixture((int_arg,))
    }
    async fn non_null_int_arg_field(&self, non_null_int_arg: i32) -> Option<String> {
        validation_fixture((non_null_int_arg,))
    }
    async fn string_arg_field(&self, string_arg: Option<String>) -> Option<String> {
        validation_fixture((string_arg,))
    }
    async fn boolean_arg_field(&self, boolean_arg: Option<bool>) -> Option<String> {
        validation_fixture((boolean_arg,))
    }
    async fn enum_arg_field(&self, enum_arg: Option<FurColor>) -> Option<String> {
        validation_fixture((enum_arg,))
    }
    async fn float_arg_field(&self, float_arg: Option<f64>) -> Option<String> {
        validation_fixture((float_arg,))
    }
    async fn id_arg_field(&self, id_arg: Option<ID>) -> Option<String> {
        validation_fixture((id_arg,))
    }
    async fn string_list_arg_field(
        &self,
        string_list_arg: Option<Vec<Option<String>>>,
    ) -> Option<String> {
        validation_fixture((string_list_arg,))
    }
    async fn complex_arg_field(&self, complex_arg: Option<ComplexInput>) -> Option<String> {
        validation_fixture((complex_arg,))
    }
    async fn multiple_reqs(&self, req1: i32, req2: i32) -> Option<String> {
        validation_fixture((req1, req2))
    }
    async fn multiple_opts(
        &self,
        #[graphql(default)] opt1: i32,
        #[graphql(default)] opt2: i32,
    ) -> Option<String> {
        validation_fixture((opt1, opt2))
    }
    async fn multiple_opt_and_req(
        &self,
        req1: i32,
        req2: i32,
        #[graphql(default)] opt1: i32,
        #[graphql(default)] opt2: i32,
    ) -> Option<String> {
        validation_fixture((req1, req2, opt1, opt2))
    }
}
#[derive(InputObject)]
#[graphql(internal, name = "OneofArg")]
struct OneofFields {
    a: Option<i32>,
    b: Option<String>,
}
struct OneofArg;
impl InputType for OneofArg {
    type RawValueType = Self;
    fn type_name() -> std::borrow::Cow<'static, str> {
        "OneofArg".into()
    }
    fn create_type_info(registry: &mut registry::Registry) -> String {
        let ty = OneofFields::create_type_info(registry);
        let registry::MetaType::InputObject { oneof, .. } =
            registry.types.get_mut("OneofArg").unwrap()
        else {
            panic!("expected input object")
        };
        *oneof = true;
        ty
    }
    fn parse(_: Option<Value>) -> InputValueResult<Self> {
        validation_fixture(())
    }
    fn to_value(&self) -> Value {
        validation_fixture(())
    }
    fn as_raw_value(&self) -> Option<&Self> {
        Some(self)
    }
}
fn validation_fixture<T>(_: impl Sized) -> T {
    panic!("validation fixtures must not execute")
}

pub struct Query;
#[Object(internal)]
impl Query {
    async fn human(&self, id: Option<ID>) -> Option<Human> {
        validation_fixture((id,))
    }
    async fn alien(&self) -> Option<Alien> {
        validation_fixture(())
    }
    async fn dog(&self) -> Option<Dog> {
        validation_fixture(())
    }
    async fn cat(&self) -> Option<Cat> {
        validation_fixture(())
    }
    async fn pet(&self) -> Option<Pet> {
        validation_fixture(())
    }
    async fn being(&self) -> Option<Being> {
        validation_fixture(())
    }
    async fn intelligent(&self) -> Option<Intelligent> {
        validation_fixture(())
    }
    async fn cat_or_dog(&self) -> Option<CatOrDog> {
        validation_fixture(())
    }
    async fn dog_or_human(&self) -> Option<DogOrHuman> {
        validation_fixture(())
    }
    async fn human_or_alien(&self) -> Option<HumanOrAlien> {
        validation_fixture(())
    }
    async fn complicated_args(&self) -> Option<ComplicatedArgs> {
        validation_fixture(())
    }
    async fn oneof_arg(&self, arg: OneofArg) -> String {
        validation_fixture((arg,))
    }
    async fn oneof_opt(&self, arg: Option<OneofArg>) -> String {
        validation_fixture((arg,))
    }
}
pub struct Mutation;
#[Object(internal)]
impl Mutation {
    async fn test_input(&self, #[graphql(default)] input: TestInput) -> i32 {
        validation_fixture((input,))
    }
}
pub struct Subscription;
#[Subscription(internal)]
impl Subscription {
    async fn values(&self) -> impl Stream<Item = i32> {
        futures_util::stream::once(async move { 10 })
    }
}
static TEST_HARNESS: OnceLock<Schema<Query, Mutation, Subscription>> = OnceLock::new();
pub(crate) fn validate<'a, V, F>(
    doc: &'a ExecutableDocument,
    factory: F,
) -> Result<(), Vec<RuleError>>
where
    V: Visitor<'a> + 'a,
    F: Fn() -> V,
{
    let schema = TEST_HARNESS.get_or_init(|| Schema::new(Query, Mutation, Subscription));
    let registry = &schema.0.env.registry;
    let mut ctx = VisitorContext::new(registry, doc, None, None);
    let mut visitor = factory();
    visit(&mut visitor, &mut ctx, doc);
    if ctx.errors.is_empty() {
        Ok(())
    } else {
        Err(ctx.errors)
    }
}
pub(crate) fn expect_passes_rule_<'a, V, F>(doc: &'a ExecutableDocument, factory: F)
where
    V: Visitor<'a> + 'a,
    F: Fn() -> V,
{
    if let Err(errors) = validate(doc, factory) {
        for err in errors {
            if let Some(position) = err.locations.first() {
                print!("[{}:{}] ", position.line, position.column);
            }
            println!("{}", err.message);
        }
        panic!("Expected rule to pass, but errors found");
    }
}
macro_rules! expect_passes_rule {
    ($ factory : expr , $ query_source : literal $ (,) ?) => {
        let doc = crate::parser::parse_query($query_source).expect("Parse error");
        crate::validation::test_harness::expect_passes_rule_(&doc, $factory);
    };
}
pub(crate) fn expect_fails_rule_<'a, V, F>(doc: &'a ExecutableDocument, factory: F)
where
    V: Visitor<'a> + 'a,
    F: Fn() -> V,
{
    if validate(doc, factory).is_ok() {
        panic!("Expected rule to fail, but no errors were found");
    }
}
macro_rules! expect_fails_rule {
    ($ factory : expr , $ query_source : literal $ (,) ?) => {
        let doc = crate::parser::parse_query($query_source).expect("Parse error");
        crate::validation::test_harness::expect_fails_rule_(&doc, $factory);
    };
}
