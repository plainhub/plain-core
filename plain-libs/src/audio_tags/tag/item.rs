use crate::audio_tags::tag::TagType;
use crate::audio_tags::tag::items::{Lang, UNKNOWN_LANGUAGE};

use std::borrow::Cow;
use std::collections::HashMap;

macro_rules! first_key {
    ($key:tt $(| $remaining:expr)*) => {
        $key
    };
}

pub(crate) use first_key;

// This is used to create the key/ItemKey maps
//
// First comes the name of the map.
// Ex:
//
// APE_MAP;
//
// This is followed by the key value pairs separated by `=>`, with the key being the
// format-specific key and the value being the appropriate ItemKey variant.
// Ex. "Artist" => Artist
//
// Some formats have multiple keys that map to the same ItemKey variant, which can be added with '|'.
// The standard key(s) **must** come before any popular non-standard keys.
// Keys should appear in order of popularity.
macro_rules! gen_map {
	($(#[$meta:meta])? $NAME:ident; $($($key:literal)|+ => $variant:ident),+) => {
		$(#[$meta])?

		struct $NAME;

		$(#[$meta])?
		impl $NAME {
			pub(crate) fn get_item_key(&self, key: &str) -> Option<ItemKey> {
				static INSTANCE: std::sync::OnceLock<HashMap<&'static str, ItemKey>> = std::sync::OnceLock::new();
				INSTANCE.get_or_init(|| {
					let mut map = HashMap::new();
					$(
						$(
							map.insert($key, ItemKey::$variant);
						)+
					)+
					map
				}).iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.clone())
			}

			pub(crate) fn get_key(&self, item_key: &ItemKey) -> Option<&str> {
				match item_key {
					$(
						ItemKey::$variant => Some(first_key!($($key)|*)),
					)+
					_ => None
				}
			}
		}
	}
}

gen_map!(
    AIFF_TEXT_MAP;

    "NAME"          => TrackTitle,
    "AUTH"          => TrackArtist,
    "(c) "          => CopyrightMessage,
    "COMM" | "ANNO" => Comment
);

gen_map!(
    APE_MAP;

    "Album"                        => AlbumTitle,
    "DiscSubtitle"                 => SetSubtitle,
    "Grouping"                     => ContentGroup,
    "Title"                        => TrackTitle,
    "Subtitle"                     => TrackSubtitle,
    "WORKTITLE"                    => Work,
    "MOVEMENTNAME"                 => Movement,
    "MOVEMENT"                     => MovementNumber,
    "MOVEMENTTOTAL"                => MovementTotal,
    "ALBUMSORT"                    => AlbumTitleSortOrder,
    "ALBUMARTISTSORT"              => AlbumArtistSortOrder,
    "TITLESORT"                    => TrackTitleSortOrder,
    "ARTISTSORT"                   => TrackArtistSortOrder,
    "Album Artist" | "ALBUMARTIST" => AlbumArtist,
    "Artist"                       => TrackArtist,
    "Artists"                      => TrackArtists,
    "Arranger"                     => Arranger,
    "Writer"                       => Writer,
    "Composer"                     => Composer,
    "Conductor"                    => Conductor,
    "Director"                     => Director,
    "Engineer"                     => Engineer,
    "Lyricist"                     => Lyricist,
    "DjMixer"                      => MixDj,
    "Mixer"                        => MixEngineer,
    "Performer"                    => Performer,
    "Producer"                     => Producer,
    "Label"                        => Label,
    "MixArtist"                    => Remixer,
    "Disc"                         => DiscNumber,
    "Disc"                         => DiscTotal,
    "Track"                        => TrackNumber,
    "Track"                        => TrackTotal,
    "Year"                         => Year,
    "ORIGINALYEAR"                 => OriginalReleaseDate,
    "RELEASEDATE"                  => ReleaseDate,
    "ISRC"                         => Isrc,
    "Barcode"                      => Barcode,
    "CatalogNumber"                => CatalogNumber,
    "Compilation"                  => FlagCompilation,
    "Media"                        => OriginalMediaType,
    "EncodedBy"                    => EncodedBy,
    "REPLAYGAIN_ALBUM_GAIN"        => ReplayGainAlbumGain,
    "REPLAYGAIN_ALBUM_PEAK"        => ReplayGainAlbumPeak,
    "REPLAYGAIN_TRACK_GAIN"        => ReplayGainTrackGain,
    "REPLAYGAIN_TRACK_PEAK"        => ReplayGainTrackPeak,
    "Genre"                        => Genre,
    "Color"                        => Color,
    "Mood"                         => Mood,
    "Copyright"                    => CopyrightMessage,
    "Comment"                      => Comment,
    "language"                     => Language,
    "Script"                       => Script,
    "Lyrics"                       => Lyrics,
    "MUSICBRAINZ_TRACKID"          => MusicBrainzRecordingId,
    "MUSICBRAINZ_RELEASETRACKID"   => MusicBrainzTrackId,
    "MUSICBRAINZ_ALBUMID"          => MusicBrainzReleaseId,
    "MUSICBRAINZ_RELEASEGROUPID"   => MusicBrainzReleaseGroupId,
    "MUSICBRAINZ_ARTISTID"         => MusicBrainzArtistId,
    "MUSICBRAINZ_ALBUMARTISTID"    => MusicBrainzReleaseArtistId,
    "MUSICBRAINZ_WORKID"           => MusicBrainzWorkId
);

gen_map!(
    ID3V2_MAP;

    "TALB"                         => AlbumTitle,
    "TSST"                         => SetSubtitle,
    "TIT1"                         => ContentGroup,
    "GRP1"                         => AppleId3v2ContentGroup,
    "TIT2"                         => TrackTitle,
    "TIT3"                         => TrackSubtitle,
    "TOAL"                         => OriginalAlbumTitle,
    "TOPE"                         => OriginalArtist,
    "TOLY"                         => OriginalLyricist,
    "TSOA"                         => AlbumTitleSortOrder,
    "TSO2"                         => AlbumArtistSortOrder,
    "TSOT"                         => TrackTitleSortOrder,
    "TSOP"                         => TrackArtistSortOrder,
    "TSOC"                         => ComposerSortOrder,
    "TPE2"                         => AlbumArtist,
    "TPE1"                         => TrackArtist,
    "ARTISTS"                      => TrackArtists,
    "TEXT"                         => Writer,
    "TCOM"                         => Composer,
    "TPE3"                         => Conductor,
    "DIRECTOR"                     => Director,
    "TEXT"                         => Lyricist,
    "TMCL"                         => MusicianCredits,
    "TPUB"                         => Publisher,
    "TPUB"                         => Label,
    "TRSN"                         => InternetRadioStationName,
    "TRSO"                         => InternetRadioStationOwner,
    "TPE4"                         => Remixer,
    "TPOS"                         => DiscNumber,
    "TPOS"                         => DiscTotal,
    "TRCK"                         => TrackNumber,
    "TRCK"                         => TrackTotal,
    "POPM"                         => Popularimeter,
    "ITUNESADVISORY"               => ParentalAdvisory,
    "TDRC"                         => RecordingDate,
    "TDOR"                         => OriginalReleaseDate,
    "TSRC"                         => Isrc,
    "BARCODE"                      => Barcode,
    "CATALOGNUMBER"                => CatalogNumber,
    "WORK"                         => Work, // ID3v2.4: TXXX:WORK (Apple uses TIT1/ContentGroup, see GRP1/AppleId3v2ContentGroup for disambiguation)
    "MVNM"                         => Movement,
    "MVIN"                         => MovementNumber,
    "MVIN"                         => MovementTotal,
    "TCMP"                         => FlagCompilation,
    "PCST"                         => FlagPodcast,
    "TFLT"                         => FileType,
    "TOWN"                         => FileOwner,
    "TDTG"                         => TaggingTime,
    "TLEN"                         => Length,
    "TOFN"                         => OriginalFileName,
    "TMED"                         => OriginalMediaType,
    "TENC"                         => EncodedBy,
    "TSSE"                         => EncoderSoftware,
    "TSSE"                         => EncoderSettings,
    "TDEN"                         => EncodingTime,
    "REPLAYGAIN_ALBUM_GAIN"        => ReplayGainAlbumGain,
    "REPLAYGAIN_ALBUM_PEAK"        => ReplayGainAlbumPeak,
    "REPLAYGAIN_TRACK_GAIN"        => ReplayGainTrackGain,
    "REPLAYGAIN_TRACK_PEAK"        => ReplayGainTrackPeak,
    "WOAF"                         => AudioFileUrl,
    "WOAS"                         => AudioSourceUrl,
    "WCOM"                         => CommercialInformationUrl,
    "WCOP"                         => CopyrightUrl,
    "WOAR"                         => TrackArtistUrl,
    "WORS"                         => RadioStationUrl,
    "WPAY"                         => PaymentUrl,
    "WPUB"                         => PublisherUrl,
    "TCON"                         => Genre,
    "TKEY"                         => InitialKey,
    "COLOR"                        => Color,
    "TMOO"                         => Mood,
    "TBPM"                         => IntegerBpm,
    "TCOP"                         => CopyrightMessage,
    "TDES"                         => PodcastDescription,
    "TCAT"                         => PodcastSeriesCategory,
    "WFED"                         => PodcastUrl,
    "TDRL"                         => ReleaseDate,
    "TGID"                         => PodcastGlobalUniqueId,
    "TKWD"                         => PodcastKeywords,
    "COMM"                         => Comment,
    "TLAN"                         => Language,
    "USLT"                         => Lyrics,
    // Mapping of MusicBrainzRecordingId is implemented as a special case
    "MusicBrainz Release Track Id" => MusicBrainzTrackId,
    "MusicBrainz Album Id"         => MusicBrainzReleaseId,
    "MusicBrainz Release Group Id" => MusicBrainzReleaseGroupId,
    "MusicBrainz Artist Id"        => MusicBrainzArtistId,
    "MusicBrainz Album Artist Id"  => MusicBrainzReleaseArtistId,
    "MusicBrainz Work Id"          => MusicBrainzWorkId
);

gen_map!(
    ILST_MAP;

    "\u{a9}alb"                                          => AlbumTitle,
    "----:com.apple.iTunes:DISCSUBTITLE"                 => SetSubtitle,
    "tvsh"                                               => ShowName,
    "\u{a9}grp"                                          => ContentGroup,
    "\u{a9}nam"                                          => TrackTitle,
    "----:com.apple.iTunes:SUBTITLE"                     => TrackSubtitle,
    "\u{a9}wrk"                                          => Work,
    "\u{a9}mvn"                                          => Movement,
    "\u{a9}mvi"                                          => MovementNumber,
    "\u{a9}mvc"                                          => MovementTotal,
    "soal"                                               => AlbumTitleSortOrder,
    "soaa"                                               => AlbumArtistSortOrder,
    "sonm"                                               => TrackTitleSortOrder,
    "soar"                                               => TrackArtistSortOrder,
    "sosn"                                               => ShowNameSortOrder,
    "soco"                                               => ComposerSortOrder,
    "aART"                                               => AlbumArtist,
    "\u{a9}ART"                                          => TrackArtist,
    "----:com.apple.iTunes:ARTISTS"                      => TrackArtists,
    "\u{a9}wrt"                                          => Composer,
    "\u{a9}dir"                                          => Director,
    "----:com.apple.iTunes:CONDUCTOR"                    => Conductor,
    "----:com.apple.iTunes:ENGINEER"                     => Engineer,
    "----:com.apple.iTunes:LYRICIST"                     => Lyricist,
    "----:com.apple.iTunes:DJMIXER"                      => MixDj,
    "----:com.apple.iTunes:MIXER"                        => MixEngineer,
    "----:com.apple.iTunes:PRODUCER"                     => Producer,
    "----:com.apple.iTunes:LABEL"                        => Label,
    "----:com.apple.iTunes:REMIXER"                      => Remixer,
    "disk"                                               => DiscNumber,
    "disk"                                               => DiscTotal,
    "trkn"                                               => TrackNumber,
    "trkn"                                               => TrackTotal,
    "rate"                                               => Popularimeter,
    "rtng"                                               => ParentalAdvisory,
    "\u{a9}day"                                          => RecordingDate,
    "----:com.apple.iTunes:ORIGINALDATE"                 => OriginalReleaseDate, // TagLib v2.0
    "----:com.apple.iTunes:RELEASEDATE"                  => ReleaseDate,
    "----:com.apple.iTunes:ISRC"                         => Isrc,
    "----:com.apple.iTunes:BARCODE"                      => Barcode,
    "----:com.apple.iTunes:CATALOGNUMBER"                => CatalogNumber,
    "cpil"                                               => FlagCompilation,
    "pcst"                                               => FlagPodcast,
    "----:com.apple.iTunes:MEDIA"                        => OriginalMediaType,
    "\u{a9}enc"                                          => EncodedBy,
    "\u{a9}too"                                          => EncoderSoftware,
    "\u{a9}gen"                                          => Genre,
    "----:com.apple.iTunes:COLOR"                        => Color,
    "----:com.apple.iTunes:MOOD"                         => Mood,
    "tmpo"                                               => IntegerBpm,
    "----:com.apple.iTunes:BPM"                          => Bpm,
    "----:com.apple.iTunes:initialkey"                   => InitialKey,
    "----:com.apple.iTunes:replaygain_album_gain"        => ReplayGainAlbumGain,
    "----:com.apple.iTunes:replaygain_album_peak"        => ReplayGainAlbumPeak,
    "----:com.apple.iTunes:replaygain_track_gain"        => ReplayGainTrackGain,
    "----:com.apple.iTunes:replaygain_track_peak"        => ReplayGainTrackPeak,
    "cprt"                                               => CopyrightMessage,
    "----:com.apple.iTunes:LICENSE"                      => License,
    "ldes"                                               => PodcastDescription,
    "catg"                                               => PodcastSeriesCategory,
    "purl"                                               => PodcastUrl,
    "egid"                                               => PodcastGlobalUniqueId,
    "keyw"                                               => PodcastKeywords,
    "\u{a9}cmt"                                          => Comment,
    "desc"                                               => Description,
    "----:com.apple.iTunes:LANGUAGE"                     => Language,
    "----:com.apple.iTunes:SCRIPT"                       => Script,
    "\u{a9}lyr"                                          => Lyrics,
    "xid "                                               => AppleXid,
    "----:com.apple.iTunes:MusicBrainz Track Id"         => MusicBrainzRecordingId,
    "----:com.apple.iTunes:MusicBrainz Release Track Id" => MusicBrainzTrackId,
    "----:com.apple.iTunes:MusicBrainz Album Id"         => MusicBrainzReleaseId,
    "----:com.apple.iTunes:MusicBrainz Release Group Id" => MusicBrainzReleaseGroupId,
    "----:com.apple.iTunes:MusicBrainz Artist Id"        => MusicBrainzArtistId,
    "----:com.apple.iTunes:MusicBrainz Album Artist Id"  => MusicBrainzReleaseArtistId,
    "----:com.apple.iTunes:MusicBrainz Work Id"          => MusicBrainzWorkId
);

gen_map!(
    RIFF_INFO_MAP;

    "IPRD"          => AlbumTitle,
    "INAM"          => TrackTitle,
    "IART"          => TrackArtist,
    "IWRI"          => Writer,
    "IMUS"          => Composer,
    "IPRO"          => Producer,
    "IPRT" | "ITRK" => TrackNumber,
    "IFRM"          => TrackTotal,
    "IRTD"          => Popularimeter,
    "ICRD"          => RecordingDate,
    "TLEN"          => Length,
    "ISRF"          => OriginalMediaType,
    "ITCH"          => EncodedBy,
    "ISFT"          => EncoderSoftware,
    "IGNR"          => Genre,
    "ICOP"          => CopyrightMessage,
    "ICMT"          => Comment,
    "ILNG"          => Language
);

gen_map!(
    VORBIS_MAP;

    "ALBUM"                                   => AlbumTitle,
    "DISCSUBTITLE"                            => SetSubtitle,
    "GROUPING"                                => ContentGroup,
    "TITLE"                                   => TrackTitle,
    "SUBTITLE"                                => TrackSubtitle,
    "WORK"                                    => Work,
    "MOVEMENTNAME"                            => Movement,
    "MOVEMENT"                                => MovementNumber,
    "MOVEMENTTOTAL"                           => MovementTotal,
    "ALBUMSORT"                               => AlbumTitleSortOrder,
    "ALBUMARTISTSORT"                         => AlbumArtistSortOrder,
    "TITLESORT"                               => TrackTitleSortOrder,
    "ARTISTSORT"                              => TrackArtistSortOrder,
    "ALBUMARTIST"                             => AlbumArtist,
    "ARTIST"                                  => TrackArtist,
    "ARTISTS"                                 => TrackArtists,
    "ARRANGER"                                => Arranger,
    "AUTHOR" | "WRITER"                       => Writer,
    "COMPOSER"                                => Composer,
    "CONDUCTOR"                               => Conductor,
    "DIRECTOR"                                => Director,
    "ENGINEER"                                => Engineer,
    "LYRICIST"                                => Lyricist,
    "DJMIXER"                                 => MixDj,
    "MIXER"                                   => MixEngineer,
    "PERFORMER"                               => Performer,
    "PRODUCER"                                => Producer,
    "PUBLISHER"                               => Publisher,
    "LABEL" | "ORGANIZATION"                  => Label,
    "REMIXER" | "MIXARTIST"                   => Remixer,
    "DISCNUMBER"                              => DiscNumber,
    "DISCTOTAL" | "TOTALDISCS"                => DiscTotal,
    "TRACKNUMBER"                             => TrackNumber,
    "TRACKTOTAL" | "TOTALTRACKS"              => TrackTotal,
    "RATING"                                  => Popularimeter,
    "DATE"                                    => RecordingDate,
    "YEAR"                                    => Year,
    "ORIGINALDATE" | "ORIGINALYEAR"           => OriginalReleaseDate,
    "RELEASEDATE"                             => ReleaseDate,
    "ISRC"                                    => Isrc,
    "BARCODE"                                 => Barcode,
    "CATALOGNUMBER"                           => CatalogNumber,
    "COMPILATION"                             => FlagCompilation,
    "MEDIA"                                   => OriginalMediaType,
    "ENCODEDBY" | "ENCODED-BY" | "ENCODED_BY" => EncodedBy,
    "ENCODER"                                 => EncoderSoftware,
    "ENCODING" | "ENCODERSETTINGS"            => EncoderSettings,
    "REPLAYGAIN_ALBUM_GAIN"                   => ReplayGainAlbumGain,
    "REPLAYGAIN_ALBUM_PEAK"                   => ReplayGainAlbumPeak,
    "REPLAYGAIN_TRACK_GAIN"                   => ReplayGainTrackGain,
    "REPLAYGAIN_TRACK_PEAK"                   => ReplayGainTrackPeak,
    "GENRE"                                   => Genre,
    "COLOR"                                   => Color,
    "MOOD"                                    => Mood,
    "BPM"                                     => Bpm,
    // MusicBrainz Picard suggests "KEY" (VirtualDJ, Denon Engine DJ), but "INITIALKEY"
    // seems to be more common (Rekordbox, Serato DJ, Traktor DJ, Mixxx).
    // <https://picard-docs.musicbrainz.org/en/appendices/tag_mapping.html#initial-key>
    // <https://github.com/beetbox/beets/issues/637#issuecomment-39528023>
    "INITIALKEY" | "KEY"                      => InitialKey,
    "COPYRIGHT"                               => CopyrightMessage,
    "LICENSE"                                 => License,
    "COMMENT"                                 => Comment,
    "LANGUAGE"                                => Language,
    "SCRIPT"                                  => Script,
    "LYRICS"                                  => Lyrics,
    "MUSICBRAINZ_TRACKID"                     => MusicBrainzRecordingId,
    "MUSICBRAINZ_RELEASETRACKID"              => MusicBrainzTrackId,
    "MUSICBRAINZ_ALBUMID"                     => MusicBrainzReleaseId,
    "MUSICBRAINZ_RELEASEGROUPID"              => MusicBrainzReleaseGroupId,
    "MUSICBRAINZ_ARTISTID"                    => MusicBrainzArtistId,
    "MUSICBRAINZ_ALBUMARTISTID"               => MusicBrainzReleaseArtistId,
    "MUSICBRAINZ_WORKID"                      => MusicBrainzWorkId
);

macro_rules! gen_item_keys {
	(
		MAPS => [
			$(
				$(#[$feat:meta])?
				[$tag_type:pat, $MAP:ident]
			),+
		];
		KEYS => [
			$(
				$(#[$variant_meta:meta])*
				$variant_ident:ident
			),+
			$(,)?
		]
	) => {
		#[derive(PartialEq, Clone, Debug, Eq, Hash)]

		#[non_exhaustive]

		pub enum ItemKey {
			$(
				$(#[$variant_meta])*
				$variant_ident,
			)+

			Unknown(String),
		}

		impl ItemKey {

			pub fn from_key(tag_type: TagType, key: &str) -> Self {
				match tag_type {
					$(
						$(#[$feat])?
						$tag_type => $MAP.get_item_key(key).unwrap_or_else(|| Self::Unknown(key.to_string())),
					)+
					_ => Self::Unknown(key.to_string())
				}
			}

			pub fn map_key(&self, tag_type: TagType, allow_unknown: bool) -> Option<&str> {
				match tag_type {
					$(
						$(#[$feat])?
						$tag_type => if let Some(key) = $MAP.get_key(self) {
							return Some(key)
						},
					)+
					_ => {}
				}

				if let ItemKey::Unknown(unknown) = self {
					if allow_unknown {
						return Some(unknown)
					}
				}

				None
			}
		}
	}
}

gen_item_keys!(
    MAPS => [
        [TagType::AiffText, AIFF_TEXT_MAP],

        [TagType::Ape, APE_MAP],

        [TagType::Id3v2, ID3V2_MAP],

        [TagType::Mp4Ilst, ILST_MAP],

        [TagType::RiffInfo, RIFF_INFO_MAP],

        [TagType::VorbisComments, VORBIS_MAP]
    ];

    KEYS => [
        // Titles
        AlbumTitle,
        SetSubtitle,
        ShowName,
        ContentGroup,
        TrackTitle,
        TrackSubtitle,

        // Original names
        OriginalAlbumTitle,
        OriginalArtist,
        OriginalLyricist,

        // Sorting
        AlbumTitleSortOrder,
        AlbumArtistSortOrder,
        TrackTitleSortOrder,
        TrackArtistSortOrder,
        ShowNameSortOrder,
        ComposerSortOrder,

        // People & Organizations
        AlbumArtist,
        TrackArtist,

        TrackArtists,
        Arranger,
        Writer,
        Composer,
        Conductor,
        Director,
        Engineer,
        Lyricist,
        MixDj,
        MixEngineer,
        MusicianCredits,
        Performer,
        Producer,
        Publisher,
        Label,
        InternetRadioStationName,
        InternetRadioStationOwner,
        Remixer,

        // Counts & Indexes
        DiscNumber,
        DiscTotal,
        TrackNumber,
        TrackTotal,
        Popularimeter,
        ParentalAdvisory,

        // Dates

        RecordingDate,

        Year,

        ReleaseDate,

        OriginalReleaseDate,

        // Identifiers
        Isrc,
        Barcode,
        CatalogNumber,
        Work,
        Movement,
        MovementNumber,
        MovementTotal,

        ///////////////////////////////////////////////////////////////
        // MusicBrainz Identifiers

        MusicBrainzRecordingId,

        MusicBrainzTrackId,

        MusicBrainzReleaseId,

        MusicBrainzReleaseGroupId,

        MusicBrainzArtistId,

        MusicBrainzReleaseArtistId,

        MusicBrainzWorkId,

        ///////////////////////////////////////////////////////////////

        // Flags
        FlagCompilation,
        FlagPodcast,

        // File Information
        FileType,
        FileOwner,
        TaggingTime,
        Length,
        OriginalFileName,
        OriginalMediaType,

        // Encoder information
        EncodedBy,
        EncoderSoftware,
        EncoderSettings,
        EncodingTime,
        ReplayGainAlbumGain,
        ReplayGainAlbumPeak,
        ReplayGainTrackGain,
        ReplayGainTrackPeak,

        // URLs
        AudioFileUrl,
        AudioSourceUrl,
        CommercialInformationUrl,
        CopyrightUrl,
        TrackArtistUrl,
        RadioStationUrl,
        PaymentUrl,
        PublisherUrl,

        // Style
        Genre,
        InitialKey,
        Color,
        Mood,

        Bpm,

        IntegerBpm,

        // Legal
        CopyrightMessage,
        License,

        // Podcast
        PodcastDescription,
        PodcastSeriesCategory,
        PodcastUrl,
        PodcastGlobalUniqueId,
        PodcastKeywords,

        // Miscellaneous
        Comment,
        Description,
        Language,
        Script,
        Lyrics,

        // Vendor-specific
        AppleXid,
        AppleId3v2ContentGroup, // GRP1
    ]
);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ItemValue {
    Text(String),

    Locator(String),

    Binary(Vec<u8>),
}

impl ItemValue {
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn locator(&self) -> Option<&str> {
        match self {
            Self::Locator(locator) => Some(locator),
            _ => None,
        }
    }

    pub fn binary(&self) -> Option<&[u8]> {
        match self {
            Self::Binary(bin) => Some(bin),
            _ => None,
        }
    }

    pub fn into_string(self) -> Option<String> {
        match self {
            Self::Text(s) | Self::Locator(s) => Some(s),
            _ => None,
        }
    }

    pub fn into_binary(self) -> Option<Vec<u8>> {
        match self {
            Self::Binary(b) => Some(b),
            _ => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Binary(binary) => binary.is_empty(),
            Self::Locator(locator) => locator.is_empty(),
            Self::Text(text) => text.is_empty(),
        }
    }
}

pub(crate) enum ItemValueRef<'a> {
    Text(Cow<'a, str>),
    Locator(&'a str),
    Binary(&'a [u8]),
}

impl<'a> Into<ItemValueRef<'a>> for &'a ItemValue {
    fn into(self) -> ItemValueRef<'a> {
        match self {
            ItemValue::Text(text) => ItemValueRef::Text(Cow::Borrowed(text)),
            ItemValue::Locator(locator) => ItemValueRef::Locator(locator),
            ItemValue::Binary(binary) => ItemValueRef::Binary(binary),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TagItem {
    pub(crate) lang: Lang,
    pub(crate) description: String,
    pub(crate) item_key: ItemKey,
    pub(crate) item_value: ItemValue,
}

impl TagItem {
    pub fn new_checked(
        tag_type: TagType,
        item_key: ItemKey,
        item_value: ItemValue,
    ) -> Option<Self> {
        item_key
            .map_key(tag_type, false)
            .is_some()
            .then_some(Self::new(item_key, item_value))
    }

    #[must_use]
    pub const fn new(item_key: ItemKey, item_value: ItemValue) -> Self {
        Self {
            lang: UNKNOWN_LANGUAGE,
            description: String::new(),
            item_key,
            item_value,
        }
    }

    pub fn set_lang(&mut self, lang: Lang) {
        self.lang = lang;
    }

    pub fn lang(&self) -> &Lang {
        &self.lang
    }

    pub fn set_description(&mut self, description: String) {
        self.description = description;
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn key(&self) -> &ItemKey {
        &self.item_key
    }

    pub fn into_key(self) -> ItemKey {
        self.item_key
    }

    pub fn value(&self) -> &ItemValue {
        &self.item_value
    }

    pub fn into_value(self) -> ItemValue {
        self.item_value
    }

    pub fn consume(self) -> (ItemKey, ItemValue) {
        (self.item_key, self.item_value)
    }

    pub(crate) fn re_map(&self, tag_type: TagType) -> bool {
        if tag_type == TagType::Id3v1 {
            use crate::audio_tags::id3::v1::constants::VALID_ITEMKEYS;

            return VALID_ITEMKEYS.contains(&self.item_key);
        }

        self.item_key.map_key(tag_type, false).is_some()
    }
}
