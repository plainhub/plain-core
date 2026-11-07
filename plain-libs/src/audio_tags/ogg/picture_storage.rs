use crate::audio_tags::error::Result;
use crate::audio_tags::picture::{Picture, PictureInformation, PictureType};

pub trait OggPictureStorage: private::Sealed {
    fn insert_picture(
        &mut self,
        picture: Picture,
        information: Option<PictureInformation>,
    ) -> Result<Option<(Picture, PictureInformation)>> {
        let ret = match picture.pic_type {
            PictureType::Icon | PictureType::OtherIcon => self
                .pictures()
                .iter()
                .position(|(p, _)| p.pic_type == picture.pic_type)
                .map(|pos| self.remove_picture(pos)),
            _ => None,
        };

        let info = match information {
            Some(pic_info) => pic_info,
            None => PictureInformation::from_picture(&picture)?,
        };

        self.pictures_mut().push((picture, info));

        Ok(ret)
    }

    fn remove_picture_type(&mut self, picture_type: PictureType) {
        self.pictures_mut()
            .retain(|(pic, _)| pic.pic_type != picture_type);
    }

    fn pictures(&self) -> &[(Picture, PictureInformation)];

    fn set_picture(&mut self, index: usize, picture: Picture, info: PictureInformation) {
        if index >= self.pictures().len() {
            // Safe to unwrap, since `info` is guaranteed to exist
            self.insert_picture(picture, Some(info)).unwrap();
        } else {
            self.pictures_mut()[index] = (picture, info);
        }
    }

    fn remove_picture(&mut self, index: usize) -> (Picture, PictureInformation) {
        self.pictures_mut().remove(index)
    }

    fn remove_pictures(&mut self) -> Vec<(Picture, PictureInformation)> {
        core::mem::take(self.pictures_mut())
    }
}

mod private {
    use crate::audio_tags::picture::{Picture, PictureInformation};

    pub trait Sealed {
        fn pictures_mut(&mut self) -> &mut Vec<(Picture, PictureInformation)>;
    }

    impl Sealed for crate::audio_tags::ogg::tag::VorbisComments {
        fn pictures_mut(&mut self) -> &mut Vec<(Picture, PictureInformation)> {
            &mut self.pictures
        }
    }
    impl Sealed for crate::audio_tags::flac::FlacFile {
        fn pictures_mut(&mut self) -> &mut Vec<(Picture, PictureInformation)> {
            &mut self.pictures
        }
    }
}
