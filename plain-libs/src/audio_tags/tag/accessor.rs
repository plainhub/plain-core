use std::borrow::Cow;

macro_rules! accessor_trait {
    ($($get:ident, $set:ident, $remove:ident: $value:ty => $owned:ty;)+) => {
        pub trait Accessor {
            $(
                fn $get(&self) -> Option<$value> { None }
                fn $set(&mut self, _value: $owned) {}
                fn $remove(&mut self) {}
            )+
        }
    };
}

accessor_trait! {
    artist, set_artist, remove_artist: Cow<'_, str> => String;
    title, set_title, remove_title: Cow<'_, str> => String;
    album, set_album, remove_album: Cow<'_, str> => String;
    genre, set_genre, remove_genre: Cow<'_, str> => String;
    track, set_track, remove_track: u32 => u32;
    track_total, set_track_total, remove_track_total: u32 => u32;
    disk, set_disk, remove_disk: u32 => u32;
    disk_total, set_disk_total, remove_disk_total: u32 => u32;
    year, set_year, remove_year: u32 => u32;
    comment, set_comment, remove_comment: Cow<'_, str> => String;
}

macro_rules! text_accessors {
	($($name:ident, $set:ident, $remove:ident => $key:literal;)+) => {
		$(
			fn $name(&self) -> Option<Cow<'_, str>> {
				self.get($key).map(Cow::Borrowed)
			}

			fn $set(&mut self, value: String) {
				self.insert(String::from($key), value)
			}

			fn $remove(&mut self) {
				let _ = self.remove($key);
			}
		)+
	}
}

pub(crate) use text_accessors;
