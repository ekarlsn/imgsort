use crate::{sorting::Tag, ImageInfo, Metadata, PreloadImage, PRELOAD_IN_FLIGHT};
use itertools::Itertools;

#[derive(Debug)]
pub struct PathList {
    pub paths: Vec<ImageInfo>,
    pub index: usize,
    pub preload_back_num: usize,
    pub preload_front_num: usize,
}

impl PathList {
    pub fn new(paths: Vec<String>, preload_back_num: usize, preload_front_num: usize) -> Self {
        let paths = paths
            .iter()
            .map(|path| ImageInfo {
                path: path.clone(),
                data: PreloadImage::Loading(path.clone()),
                metadata: Metadata { tag: None },
            })
            .collect();
        Self {
            paths,
            index: 0,
            preload_back_num,
            preload_front_num,
        }
    }

    // Preload order?
    // cache-size = 100, how many picture are kept in the list, when you scroll past preload limit
    // back = 10, how many you start preloading backwards
    // front = 30, how many you start preloading forwards
    // in_flight = 8 (Or number of cores?), how many you preload at the same time
    pub fn get_initial_preload_images(&self) -> Vec<String> {
        let mut paths = Vec::new();
        let from = self
            .index
            .saturating_sub(std::cmp::min(self.preload_back_num, PRELOAD_IN_FLIGHT / 2));
        let to = *[
            self.index + self.preload_front_num + 1,
            self.paths.len(),
            from + PRELOAD_IN_FLIGHT,
        ]
        .iter()
        .min()
        .expect("The iter is not empty");

        for i in from..to {
            paths.push(self.paths[i].path.clone());
        }
        paths
    }

    pub fn tag_of(&self, path: &str) -> Option<Tag> {
        self.paths
            .iter()
            .find(|info| info.path == path)
            .and_then(|info| info.metadata.tag)
    }

    pub fn prev(&self) -> Option<&ImageInfo> {
        if self.index == 0 {
            None
        } else {
            Some(&self.paths[self.index - 1])
        }
    }

    pub fn current(&self) -> &ImageInfo {
        &self.paths[self.index]
    }

    pub fn next(&self) -> Option<&ImageInfo> {
        self.paths.get(self.index + 1)
    }

    pub fn current_mut(&mut self) -> &mut ImageInfo {
        &mut self.paths[self.index]
    }
}

pub fn schedule_next_preload_image_after_one_finished(pathlist: &PathList) -> Option<String> {
    let curr = pathlist.index;

    let forward = pathlist.paths.iter().skip(curr);
    let rev = pathlist
        .paths
        .iter()
        .rev()
        .skip(pathlist.paths.len() - curr);

    for e in forward.interleave(rev) {
        let loading = match e.data {
            PreloadImage::Loading(_) => true,
            _ => false,
        };
        if loading {
            return Some(e.path.clone());
        }
    }
    None
}
