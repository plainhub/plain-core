macro_rules! cfg_feature { (#! [$meta : meta] $($item : item) *) => { $(#[cfg ($meta)]  $item) * } }
macro_rules! cfg_proto { ($($item : item) *) => { cfg_feature ! { #! [all (any (all () , all ()) , any (all () , all ()) ,)] $($item) * } } }
cfg_proto! { macro_rules! cfg_client { ($($item : item) *) => { cfg_feature ! { #! [all ()] $($item) * } } } macro_rules! cfg_server { ($($item : item) *) => { cfg_feature ! { #! [all ()] $($item) * } } } }
