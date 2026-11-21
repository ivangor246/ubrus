mod gif;
mod thumbnail;
mod video;
pub use thumbnail::thumbnail;
pub use video::Video;

use anyhow::Result;
use gst::prelude::*;
use gtk::gio::{self, prelude::FileExt};
use std::path::Path;

pub fn init() -> Result<()> {
    gst::init()?;
    gstgtk4::plugin_register_static()?;
    Ok(())
}

/// A silent playbin that releases its decoder when the last owning operation ends.
pub(crate) struct VideoPipeline(gst::Element);

impl VideoPipeline {
    pub fn open(path: &Path, sink: &gst::Element) -> Result<Self> {
        let pipeline = gst::ElementFactory::make("playbin3").build()?;
        pipeline.set_property_from_str("flags", "video");
        pipeline.set_property("video-sink", sink);
        pipeline.set_property("uri", gio::File::for_path(path).uri().as_str());
        Ok(Self(pipeline))
    }
}

impl std::ops::Deref for VideoPipeline {
    type Target = gst::Element;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for VideoPipeline {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}
