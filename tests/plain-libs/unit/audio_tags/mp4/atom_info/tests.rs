use super::*;

// Verify that we could create freeform AtomIdent constants

const FREEFORM_ATOM_IDENT: AtomIdent<'_> = AtomIdent::Freeform {
    mean: Cow::Borrowed("mean"),
    name: Cow::Borrowed("name"),
};
