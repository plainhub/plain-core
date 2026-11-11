#[macro_export]
macro_rules! plain_indexmap_indexmap_with_default { () => { const { $crate::indexmap::IndexMap::with_hasher (::core::hash::BuildHasherDefault::new () ,) } } ; ($H : ty $(;) ?) => { const { $crate::indexmap::IndexMap::with_hasher (::core::hash::BuildHasherDefault::<$H >::new () ,) } } ; ($H : ty ; $($key : expr => $value : expr) ,+ $(,) ?) => { { let mut map = $crate::indexmap::IndexMap::with_capacity_and_hasher (const { < [()] >::len (& [$({ stringify ! ($key) ; }) ,*]) } ,::core::hash::BuildHasherDefault::<$H >::new () ,) ; $(map . insert ($key , $value) ;) + map } } ; }
#[macro_export]
macro_rules! plain_indexmap_indexmap { () => { $crate::indexmap::IndexMap::new () } ; ($($key : expr => $value : expr) ,+ $(,) ?) => { { let mut map = $crate::indexmap::IndexMap::with_capacity (const { < [()] >::len (& [$({ stringify ! ($key) ; }) ,*]) } ,) ; $(map . insert ($key , $value) ;) + map } } ; }
#[macro_export]
macro_rules! plain_indexmap_indexset_with_default { () => { const { $crate::indexmap::IndexSet::with_hasher (::core::hash::BuildHasherDefault::new () ,) } } ; ($H : ty $(;) ?) => { const { $crate::indexmap::IndexSet::with_hasher (::core::hash::BuildHasherDefault::<$H >::new () ,) } } ; ($H : ty ; $($value : expr) ,+ $(,) ?) => { { let mut set = $crate::indexmap::IndexSet::with_capacity_and_hasher (const { < [()] >::len (& [$({ stringify ! ($value) ; }) ,*]) } ,::core::hash::BuildHasherDefault::<$H >::new () ,) ; $(set . insert ($value) ;) + set } } ; }
#[macro_export]
macro_rules! plain_indexmap_indexset { () => { $crate::indexmap::IndexSet::new () } ; ($($value : expr) ,+ $(,) ?) => { { let mut set = $crate::indexmap::IndexSet::with_capacity (const { < [()] >::len (& [$({ stringify ! ($value) ; }) ,*]) } ,) ; $(set . insert ($value) ;) + set } } ; }
macro_rules! iterator_methods {
    ($map_elt : expr) => {
        fn next(&mut self) -> Option<Self::Item> {
            self.iter.next().map($map_elt)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            self.iter.size_hint()
        }
        fn count(self) -> usize {
            self.iter.len()
        }
        fn nth(&mut self, n: usize) -> Option<Self::Item> {
            self.iter.nth(n).map($map_elt)
        }
        fn last(mut self) -> Option<Self::Item> {
            self.next_back()
        }
        fn collect<C>(self) -> C
        where
            C: FromIterator<Self::Item>,
        {
            self.iter.map($map_elt).collect()
        }
    };
}
macro_rules! double_ended_iterator_methods {
    ($map_elt : expr) => {
        fn next_back(&mut self) -> Option<Self::Item> {
            self.iter.next_back().map($map_elt)
        }
        fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
            self.iter.nth_back(n).map($map_elt)
        }
    };
}
